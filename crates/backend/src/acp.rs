use pasta_common::ipc::{ChatEventType, Event};

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process;
use tokio::time::Duration;

use crate::EventTx;

pub struct AcpHandle {
    pub child: process::Child,
    pub stdin: process::ChildStdin,
    pub next_id: u64,
    pub session_id: String,
}

pub async fn spawn(agent: &str, tx: EventTx) -> anyhow::Result<AcpHandle> {
    let kiro = crate::fetchers::resolve_binary("kiro-cli", "KIRO_CLI_PATH", "kiro-cli");
    let mut child = process::Command::new(&kiro)
        .args(["acp", "--agent", agent])
        .current_dir(pasta_common::vault::vault_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize — send and wait for response
    let mut handle = AcpHandle { child, stdin, next_id: 0, session_id: String::new() };
    let init_msg = serde_json::json!({
        "jsonrpc": "2.0", "id": handle.next_id,
        "method": "initialize",
        "params": { "protocolVersion": 1, "clientCapabilities": { "fs": { "readTextFile": true, "writeTextFile": true }, "terminal": true }, "clientInfo": { "name": "pasta-backend", "version": "0.1.0" } }
    });
    let init_id = handle.next_id;
    handle.next_id += 1;
    send_jsonrpc(&mut handle, &init_msg).await?;
    await_response(&mut reader, init_id).await?;

    let session_msg = serde_json::json!({
        "jsonrpc": "2.0", "id": handle.next_id,
        "method": "session/new",
        "params": { "cwd": pasta_common::vault::vault_path(), "mcpServers": [] }
    });
    let session_id_num = handle.next_id;
    handle.next_id += 1;
    send_jsonrpc(&mut handle, &session_msg).await?;
    let session_resp = await_response_value(&mut reader, session_id_num).await?;
    handle.session_id = session_resp.get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .unwrap_or("default")
        .to_string();

    // Hand off reader to notification parsing task
    tokio::spawn(async move {
        let mut reader = reader;
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(evt) = parse_notification(&msg) {
                            let _ = tx.send(evt);
                        }
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(Event::Chat { chat_type: ChatEventType::TurnEnd });
    });

    Ok(handle)
}

async fn await_response(reader: &mut BufReader<process::ChildStdout>, expected_id: u64) -> anyhow::Result<()> {
    await_response_value(reader, expected_id).await.map(|_| ())
}

async fn await_response_value(reader: &mut BufReader<process::ChildStdout>, expected_id: u64) -> anyhow::Result<serde_json::Value> {
    let timeout = Duration::from_secs(10);
    let mut line = String::new();
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        line.clear();
        let read_fut = reader.read_line(&mut line);
        match tokio::time::timeout_at(deadline, read_fut).await {
            Ok(Ok(0)) => return Err(anyhow::anyhow!("ACP process exited during init")),
            Ok(Ok(_)) => {
                if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
                    if msg.get("id").and_then(|v| v.as_u64()) == Some(expected_id) {
                        return Ok(msg);
                    }
                }
            }
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => return Err(anyhow::anyhow!("ACP initialization timed out (10s)")),
        }
    }
}

pub async fn send_prompt(handle: &mut AcpHandle, text: &str) -> anyhow::Result<()> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": handle.next_id,
        "method": "session/prompt",
        "params": { "sessionId": handle.session_id, "prompt": [{ "type": "text", "text": text }] }
    });
    handle.next_id += 1;
    send_jsonrpc(handle, &msg).await
}

async fn send_jsonrpc(handle: &mut AcpHandle, msg: &serde_json::Value) -> anyhow::Result<()> {
    let mut s = serde_json::to_string(msg)?;
    s.push('\n');
    handle.stdin.write_all(s.as_bytes()).await?;
    handle.stdin.flush().await?;
    Ok(())
}

fn parse_notification(msg: &serde_json::Value) -> Option<Event> {
    let method = msg.get("method")?.as_str()?;

    // Kiro uses "session/update" with update.sessionUpdate field
    if method == "session/update" {
        let params = msg.get("params")?;
        let update = params.get("update")?;
        let update_type = update.get("sessionUpdate")?.as_str()?;
        return match update_type {
            "agent_message_chunk" => {
                let text = update.get("content")?.get("text")?.as_str()?;
                Some(Event::Chat { chat_type: ChatEventType::Text { text: text.to_string() } })
            }
            "tool_call" | "tool_use" => {
                let name = update.get("name")
                    .or_else(|| update.get("toolName"))
                    .and_then(|n| n.as_str()).unwrap_or("unknown").to_string();
                let status = update.get("status").and_then(|s| s.as_str()).unwrap_or("running").to_string();
                Some(Event::Chat { chat_type: ChatEventType::ToolCall { name, status } })
            }
            "tool_result" => {
                let name = update.get("name")
                    .or_else(|| update.get("toolName"))
                    .and_then(|n| n.as_str()).unwrap_or("tool").to_string();
                Some(Event::Chat { chat_type: ChatEventType::ToolCall { name, status: "done".to_string() } })
            }
            "turn_end" | "turn_complete" => Some(Event::Chat { chat_type: ChatEventType::TurnEnd }),
            _ => {
                tracing::debug!(update_type, "acp: unhandled session/update type");
                None
            }
        };
    }

    // Legacy format: "session/notification" with params.type
    if method == "session/notification" {
        let params = msg.get("params")?;
        let update_type = params.get("type")?.as_str()?;
        return match update_type {
            "AgentMessageChunk" => {
                let text = params.get("content")?.get("text")?.as_str()?;
                Some(Event::Chat { chat_type: ChatEventType::Text { text: text.to_string() } })
            }
            "ToolCall" | "ToolUse" => {
                let name = params.get("name")
                    .or_else(|| params.get("toolName"))
                    .and_then(|n| n.as_str()).unwrap_or("unknown").to_string();
                let status = params.get("status").and_then(|s| s.as_str()).unwrap_or("running").to_string();
                Some(Event::Chat { chat_type: ChatEventType::ToolCall { name, status } })
            }
            "ToolResult" => {
                let name = params.get("name")
                    .or_else(|| params.get("toolName"))
                    .and_then(|n| n.as_str()).unwrap_or("tool").to_string();
                Some(Event::Chat { chat_type: ChatEventType::ToolCall { name, status: "done".to_string() } })
            }
            "TurnEnd" | "TurnComplete" => Some(Event::Chat { chat_type: ChatEventType::TurnEnd }),
            _ => {
                tracing::debug!(update_type, "acp: unhandled notification type");
                None
            }
        };
    }

    None
}
