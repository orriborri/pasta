//! Immutable original bytes and replayable manifests, outside disposable indexes.
use anyhow::{bail, Context, Result};
use kb_core::{Kind, Record, Source};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct AttachmentPart {
    pub path: String,
    pub part_id: String,
    pub filename: String,
    pub mime_type: String,
    pub declared_charsets: Vec<String>,
    pub size: Option<u64>,
    pub attachment_id: Option<String>,
    pub inline_data: Option<String>,
}

pub(super) fn discover(full: &Value) -> Result<Vec<AttachmentPart>> {
    let payload = full
        .pointer("/message/payload")
        .context("missing Gmail MIME payload")?;
    let mut parts = Vec::new();
    discover_part(payload, "0", &mut parts)?;
    Ok(parts)
}

fn discover_part(value: &Value, path: &str, output: &mut Vec<AttachmentPart>) -> Result<()> {
    let part: MimePart =
        serde_json::from_value(value.clone()).context("invalid Gmail MIME part")?;
    let mime = part
        .mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let mut declared_charsets = Vec::new();
    for content_type in std::iter::once(part.mime_type.as_str()).chain(
        part.headers
            .iter()
            .filter(|header| header.name.eq_ignore_ascii_case("Content-Type"))
            .map(|header| header.value.as_str()),
    ) {
        if let Some(charset) = mailparse::parse_content_type(content_type)
            .params
            .get("charset")
        {
            declared_charsets.push(charset.trim().to_ascii_lowercase());
        }
    }
    declared_charsets.sort();
    declared_charsets.dedup();
    let attachment = !part.filename.is_empty()
        || part.body.attachment_id.is_some()
        || part.headers.iter().any(|header| {
            (header.name.eq_ignore_ascii_case("Content-Disposition")
                && header
                    .value
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("attachment"))
                || (header.name.eq_ignore_ascii_case("Content-ID")
                    && !header.value.trim().is_empty())
        })
        || (!mime.starts_with("text/")
            && !mime.starts_with("multipart/")
            && (part.body.data.is_some() || part.body.size.is_some_and(|size| size > 0)));
    if attachment {
        if !part.parts.is_empty() && part.body.attachment_id.is_none() && part.body.data.is_none() {
            bail!("MIME container attachment has no retrievable original bytes");
        }
        output.push(AttachmentPart {
            path: path.into(),
            part_id: part.part_id,
            filename: part.filename,
            mime_type: part.mime_type,
            declared_charsets,
            size: part.body.size,
            attachment_id: part.body.attachment_id,
            inline_data: part.body.data,
        });
    } else {
        for (index, child) in part.parts.iter().enumerate() {
            discover_part(child, &format!("{path}.{index}"), output)?;
        }
    }
    Ok(())
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MimePart {
    #[serde(default)]
    part_id: String,
    #[serde(default)]
    filename: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    body: PartBody,
    #[serde(default)]
    headers: Vec<Header>,
    #[serde(default)]
    parts: Vec<Value>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartBody {
    size: Option<u64>,
    attachment_id: Option<String>,
    data: Option<String>,
}
#[derive(Deserialize)]
struct Header {
    #[serde(default)]
    name: String,
    #[serde(default)]
    value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Blob {
    pub sha256: String,
    pub size: u64,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    key: String,
    blob: Blob,
}
#[derive(Serialize, Deserialize)]
struct AttachmentReference {
    part_path: String,
    blob: Blob,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u8,
    message: Record,
    attachments: Vec<AttachmentReference>,
}

pub(super) struct EvidenceStore {
    root: PathBuf,
}
impl EvidenceStore {
    pub fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            bail!("Gmail evidence directory must be an absolute local storage path");
        }
        for directory in ["blobs", "receipts", "messages", "pending"] {
            create_directory_durable(&root.join(directory))?;
        }
        Ok(Self { root: root.into() })
    }

    pub fn pending(&self) -> Result<tempfile::TempDir> {
        Ok(tempfile::tempdir_in(self.root.join("pending"))?)
    }

    pub fn blob_path(&self, blob: &Blob) -> Result<PathBuf> {
        validate_digest(&blob.sha256)?;
        Ok(self.root.join("blobs").join(&blob.sha256))
    }

    fn verify(&self, blob: &Blob) -> Result<PathBuf> {
        let path = self.blob_path(blob)?;
        let actual = fingerprint(&path)?;
        if actual.sha256 != blob.sha256 || actual.size != blob.size {
            bail!("retained Gmail attachment failed checksum/size verification");
        }
        Ok(path)
    }

    pub fn cached(
        &self,
        account: &str,
        message: &str,
        part: &AttachmentPart,
    ) -> Result<Option<Blob>> {
        let key = source_key(account, message, part)?;
        let path = self.root.join("receipts").join(format!("{key}.json"));
        if !path.exists() {
            return Ok(None);
        }
        let receipt: Receipt = serde_json::from_slice(&fs::read(path)?)?;
        if receipt.key != key {
            bail!("Gmail attachment receipt identity mismatch");
        }
        verify_size(part, &receipt.blob)?;
        self.verify(&receipt.blob)?;
        sync_directory(&self.root.join("blobs"))?;
        sync_directory(&self.root.join("receipts"))?;
        Ok(Some(receipt.blob))
    }

    pub fn retain(
        &self,
        account: &str,
        message: &str,
        part: &AttachmentPart,
        source: &Path,
    ) -> Result<Blob> {
        // Copy to an owner-only temp file on the same filesystem before publishing.
        let mut temporary = tempfile::NamedTempFile::new_in(self.root.join("blobs"))?;
        std::io::copy(&mut fs::File::open(source)?, &mut temporary)?;
        temporary.as_file().sync_all()?;
        let blob = fingerprint(temporary.path())?;
        verify_size(part, &blob)?;
        let path = self.blob_path(&blob)?;
        match temporary.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.verify(&blob)?;
            }
            Err(error) => return Err(error.error.into()),
        }
        sync_directory(path.parent().context("missing blob parent")?)?;
        let key = source_key(account, message, part)?;
        publish_json(
            &self.root.join("receipts").join(format!("{key}.json")),
            &Receipt {
                key,
                blob: blob.clone(),
            },
        )?;
        Ok(blob)
    }

    pub fn finish(
        &self,
        message: Record,
        attachments: Vec<(AttachmentPart, Blob)>,
    ) -> Result<Vec<Record>> {
        let manifest = Manifest {
            version: 1,
            message,
            attachments: attachments
                .into_iter()
                .map(|(part, blob)| AttachmentReference {
                    part_path: part.path,
                    blob,
                })
                .collect(),
        };
        let records = self.project(&manifest)?;
        publish_json(
            &self
                .root
                .join("messages")
                .join(format!("{}.json", manifest.message.id)),
            &manifest,
        )?;
        Ok(records)
    }

    fn project(&self, manifest: &Manifest) -> Result<Vec<Record>> {
        if manifest.version != 1 {
            bail!("unsupported Gmail evidence manifest version");
        }
        let message = &manifest.message;
        let expected = Record::make_id(
            Source::Gmail,
            &format!("message-{:x}", Sha256::digest(message.content.as_bytes())),
        );
        if message.id != expected || message.source != Source::Gmail {
            bail!("Gmail message snapshot checksum mismatch");
        }
        let snapshot: Value = serde_json::from_str(&message.content)?;
        let parts = discover(
            snapshot
                .get("full_message")
                .context("missing retained Gmail source")?,
        )?;
        if parts.len() != manifest.attachments.len() {
            bail!("Gmail attachment manifest is incomplete");
        }
        let mut parent = message.clone();
        parent.tags.retain(|tag| tag != "attachments:metadata-only");
        parent.tags.push(
            if parts.is_empty() {
                "attachments:none-reported"
            } else {
                "attachments:retained"
            }
            .into(),
        );
        parent.tags.sort();
        parent.tags.dedup();
        let mut records = vec![parent];
        for (part, reference) in parts.iter().zip(&manifest.attachments) {
            if part.path != reference.part_path {
                bail!("Gmail attachment part identity mismatch");
            }
            verify_size(part, &reference.blob)?;
            let path = self.verify(&reference.blob)?;
            let (status, text) = extract_text(part, &reference.blob, &path)?;
            let version = source_key(&message.id, &reference.blob.sha256, part)?;
            records.push(Record {
                id: Record::make_id(Source::Gmail, &format!("attachment-{version}")), source: Source::Gmail, kind: Kind::Doc,
                title: if part.filename.is_empty() { format!("Attachment {}", part.path) } else { part.filename.clone() },
                content: serde_json::to_string(&json!({"schema":"pasta.gmail.attachment.v1", "parent_record_id":message.id,
                    "account":snapshot["account"], "message_id":snapshot["message_id"], "part_id":part.part_id, "part_path":part.path,
                    "filename":part.filename, "mime_type":part.mime_type, "declared_charsets":part.declared_charsets, "sha256":reference.blob.sha256,
                    "size":reference.blob.size, "blob_ref":format!("gmail-blob:{}", reference.blob.sha256),
                    "extraction":{"status":status,"text":text}, "indexing":"not_verified"}))?,
                author: message.author.clone(), participants: message.participants.clone(),
                created_at: message.created_at, updated_at: message.updated_at, url: message.url.clone(), thread_id: message.thread_id.clone(),
                entities: vec![], tags: vec!["evidence:gmail-attachment".into(), "retention:verified".into(), format!("extraction:{status}"), format!("parent-record:{}", message.id)],
            });
        }
        Ok(records)
    }
}

pub(super) fn read_records(root: &Path) -> Result<Vec<Record>> {
    if !root.is_absolute() || !root.is_dir() {
        bail!("configured Gmail evidence directory is missing or not absolute");
    }
    let store = EvidenceStore { root: root.into() };
    let directory = root.join("messages");
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut records = Vec::new();
    for path in paths
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
    {
        let manifest: Manifest = serde_json::from_slice(&fs::read(&path)?)?;
        if path.file_stem().and_then(|s| s.to_str()) != Some(manifest.message.id.as_str()) {
            bail!("Gmail evidence filename identity mismatch");
        }
        records.extend(store.project(&manifest)?);
    }
    Ok(records)
}

fn source_key(account: &str, message: &str, part: &AttachmentPart) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(account, message, part))?)
    ))
}
fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("invalid Gmail blob checksum");
    }
    Ok(())
}
fn verify_size(part: &AttachmentPart, blob: &Blob) -> Result<()> {
    if part.size.is_some_and(|size| size != blob.size) {
        bail!("Gmail attachment size differs from source metadata");
    }
    Ok(())
}
fn fingerprint(path: &Path) -> Result<Blob> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        bail!("Gmail evidence blob is not a regular file");
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0; 16384];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        size += count as u64;
    }
    Ok(Blob {
        sha256: format!("{:x}", hash.finalize()),
        size,
    })
}
fn extract_text(
    part: &AttachmentPart,
    blob: &Blob,
    path: &Path,
) -> Result<(&'static str, Option<String>)> {
    let mime = part
        .mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if ![
        "text/plain",
        "text/markdown",
        "text/csv",
        "text/tab-separated-values",
        "application/json",
    ]
    .contains(&mime.as_str())
    {
        return Ok(("unsupported_mime", None));
    }
    if blob.size > 1024 * 1024 {
        return Ok(("too_large", None));
    }
    if part
        .declared_charsets
        .iter()
        .any(|charset| !["utf-8", "utf8", "us-ascii", "ascii"].contains(&charset.as_str()))
    {
        return Ok(("unsupported_encoding", None));
    }
    let bytes = fs::read(path)?;
    if part
        .declared_charsets
        .iter()
        .any(|charset| charset == "us-ascii" || charset == "ascii")
        && !bytes.is_ascii()
    {
        return Ok(("unsupported_encoding", None));
    }
    match String::from_utf8(bytes) {
        Ok(text) if !text.contains('\0') => Ok(("utf8_text", Some(text))),
        _ => Ok(("unsupported_encoding", None)),
    }
}
fn publish_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if path.exists() {
        if fs::read(path)? != bytes {
            bail!("conflicting immutable Gmail evidence manifest/receipt");
        }
        return sync_directory(path.parent().context("missing evidence directory")?);
    }
    let parent = path.parent().context("missing evidence directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(path) {
        Ok(_) => sync_directory(parent),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::read(path)? != bytes {
                bail!("concurrent conflicting Gmail evidence write");
            }
            sync_directory(parent)
        }
        Err(error) => Err(error.error.into()),
    }
}

fn create_directory_durable(path: &Path) -> Result<()> {
    if !path.is_dir() {
        let parent = path
            .parent()
            .context("missing evidence directory ancestor")?;
        create_directory_durable(parent)?;
        match fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
            Err(error) => return Err(error.into()),
        }
    }
    sync_directory(path)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
