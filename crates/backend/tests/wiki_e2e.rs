//! End-to-end test: wiki sync → chunk → embed → search
//! Run with: cargo test --package pasta-backend --test wiki_e2e -- --nocapture

use std::fs;
use std::path::Path;

const VAULT: &str = "/home/orre/Obsidian/Readpeak";
const WIKI_SRC: &str = "/home/orre/ReadPeak/wiki";

#[tokio::test]
async fn wiki_sync_chunk_embed_search() {
    // 1. Verify wiki source exists
    let wiki_files: Vec<_> = fs::read_dir(WIKI_SRC)
        .expect("Wiki source dir must exist")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .collect();
    println!("✓ Wiki source: {} markdown files", wiki_files.len());
    assert!(wiki_files.len() > 10, "Expected many wiki files");

    // 2. Run wiki sync (copy files to .history/wiki/)
    let history_wiki = Path::new(VAULT).join(".history/wiki");
    fs::create_dir_all(&history_wiki).unwrap();

    let mut synced = 0;
    for entry in &wiki_files {
        let src = entry.path();
        let fname = src.file_name().unwrap();
        let dest = history_wiki.join(fname);
        let content = fs::read_to_string(&src).unwrap_or_default();
        let with_fm = if content.starts_with("---") {
            content
        } else {
            format!("---\nsource: wiki\ntype: documentation\nparticipants: []\nlast_updated: 2026-06-12\n---\n\n{}", content)
        };
        fs::write(&dest, with_fm).unwrap();
        synced += 1;
    }
    println!("✓ Wiki sync: {} files copied to .history/wiki/", synced);

    // 3. Chunk a sample file
    let sample = history_wiki.join(wiki_files[0].file_name());
    let sample_content = fs::read_to_string(&sample).unwrap();
    let chunks = chunk_text(&sample_content, 2000, 200);
    println!("✓ Chunking: '{}' → {} chunks", wiki_files[0].file_name().to_string_lossy(), chunks.len());
    assert!(!chunks.is_empty());

    // 4. Embed a few chunks via Ollama
    let test_chunks: Vec<String> = chunks.into_iter().take(3).collect();
    let embeddings = embed_ollama(&test_chunks).await;
    match &embeddings {
        Ok(vecs) => {
            println!("✓ Embedding: {} vectors, dims={}", vecs.len(), vecs[0].len());
            assert_eq!(vecs.len(), test_chunks.len());
            assert_eq!(vecs[0].len(), 1536); // padded
        }
        Err(e) => panic!("Embedding failed: {}", e),
    }

    // 5. Store in LanceDB and search
    let db_path = Path::new(VAULT).join(".lancedb");
    fs::create_dir_all(&db_path).unwrap();

    use arrow_array::{RecordBatch, StringArray, Float32Array, FixedSizeListArray, ArrayRef};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;
    use lancedb::connect;
    use lancedb::query::{ExecutableQuery, QueryBase};
    use futures::TryStreamExt;

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("path", DataType::Utf8, false),
        Field::new("participants", DataType::Utf8, false),
        Field::new("date", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("vector", DataType::FixedSizeList(
            Arc::new(Field::new("item", DataType::Float32, true)), 1536
        ), false),
    ]));

    let vecs = embeddings.unwrap();
    let flat: Vec<f32> = vecs.into_iter().flatten().collect();
    let vector_array = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        1536,
        Arc::new(Float32Array::from(flat)) as ArrayRef,
        None,
    ).unwrap();

    let ids: Vec<String> = (0..test_chunks.len()).map(|i| format!("wiki/test#{}",i)).collect();
    let batch = RecordBatch::try_new(schema.clone(), vec![
        Arc::new(StringArray::from(ids)),
        Arc::new(StringArray::from(vec!["wiki"; test_chunks.len()])),
        Arc::new(StringArray::from(vec!["wiki/test.md"; test_chunks.len()])),
        Arc::new(StringArray::from(vec![""; test_chunks.len()])),
        Arc::new(StringArray::from(vec!["2026-06-12"; test_chunks.len()])),
        Arc::new(StringArray::from(test_chunks.clone())),
        Arc::new(vector_array) as ArrayRef,
    ]).unwrap();

    let db = connect(db_path.to_str().unwrap()).execute().await.unwrap();
    // Drop existing test table if present
    db.drop_table("chunks_test", &[]).await.ok();
    let table = db.create_table("chunks_test", vec![batch]).execute().await.unwrap();
    println!("✓ LanceDB: table created with {} rows", test_chunks.len());

    // 6. Search
    let query_vec = embed_ollama(&["architecture".to_string()].to_vec()).await.unwrap();
    let qv = query_vec.into_iter().next().unwrap();
    let results: Vec<RecordBatch> = table.vector_search(qv).unwrap()
        .limit(3)
        .execute().await.unwrap()
        .try_collect().await.unwrap();

    let mut found = 0;
    for batch in &results {
        let contents = batch.column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        if let Some(c) = contents {
            for i in 0..batch.num_rows() {
                let snippet: String = c.value(i).chars().take(100).collect();
                println!("  result {}: {}...", found + 1, snippet);
                found += 1;
            }
        }
    }
    println!("✓ Search: returned {} results", found);
    assert!(found > 0, "Search should return results");

    // Cleanup test table
    db.drop_table("chunks_test", &[]).await.ok();
    println!("\n✓ End-to-end pipeline: PASS");
}

fn chunk_text(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + max_chars).min(text.len());
        let slice = &text[start..end];
        let break_at = slice.rfind("\n\n")
            .or_else(|| slice.rfind('\n'))
            .or_else(|| slice.rfind(". "))
            .map(|i| i + 1)
            .unwrap_or(slice.len());
        let chunk_end = start + break_at;
        chunks.push(text[start..chunk_end].to_string());
        start = if chunk_end > overlap { chunk_end - overlap } else { chunk_end };
        if start >= text.len() { break; }
    }
    chunks
}

async fn embed_ollama(texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
    #[derive(serde::Serialize)]
    struct Req<'a> { model: &'a str, input: &'a [String] }
    #[derive(serde::Deserialize)]
    struct Resp { embeddings: Vec<Vec<f32>> }

    let resp: Resp = reqwest::Client::new()
        .post("http://localhost:11434/api/embed")
        .json(&Req { model: "nomic-embed-text", input: texts })
        .send().await?
        .json().await?;
    Ok(resp.embeddings.into_iter().map(|mut v| { v.resize(1536, 0.0); v }).collect())
}
