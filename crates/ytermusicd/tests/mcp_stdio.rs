use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    time::timeout,
};
use ytermusicd_api::{
    AuthState, AuthStatus, DownloadSummary, PlaybackSnapshot, PlaybackState, Status, Track,
};

async fn mock_daemon() -> String {
    let status = Status {
        name: "ytermusicd".to_string(),
        version: "0.1.0".to_string(),
        status: "ok".to_string(),
        uptime_seconds: 1,
        auth: AuthStatus {
            state: AuthState::Ok,
            source: None,
            message: None,
        },
        playback: PlaybackSnapshot {
            state: PlaybackState::Playing,
            current: Some(Track {
                video_id: "v1".to_string(),
                title: "Song".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                duration: "3:00".to_string(),
            }),
            position_secs: 12.0,
            volume: 50,
            repeat: Default::default(),
            shuffle: false,
        },
        queue_length: 1,
        downloads: DownloadSummary::default(),
    };
    let router = Router::new().route(
        "/api/status",
        get(move || {
            let status = status.clone();
            async move { Json(status) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{address}")
}

#[tokio::test]
async fn mcp_stdio_lists_tools_and_exits_on_eof() {
    let base = mock_daemon().await;
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ytermusicd"))
        .args(["mcp", "--url", &base])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ytermusicd mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();

    let messages = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0.0.0" }
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "now_playing", "arguments": {} }
        }),
    ];

    for message in messages {
        stdin
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
        stdin.flush().await.unwrap();
    }

    let mut responses: Vec<Value> = Vec::new();
    loop {
        let line = timeout(Duration::from_secs(10), lines.next_line())
            .await
            .expect("MCP response timeout")
            .unwrap();
        let Some(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response: Value = serde_json::from_str(&line).expect("valid JSON response");
        responses.push(response);
        if responses.iter().any(|response| response["id"] == 3) {
            break;
        }
    }

    drop(stdin);
    let status = timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("MCP process did not exit")
        .unwrap();
    assert!(status.success(), "MCP process failed: {status}");

    let initialize = responses
        .iter()
        .find(|response| response["id"] == 1)
        .expect("initialize response");
    assert_eq!(initialize["result"]["protocolVersion"], "2025-06-18");

    let tools = responses
        .iter()
        .find(|response| response["id"] == 2)
        .expect("tools/list response");
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 13);
    assert!(names.contains(&"search"));
    assert!(names.contains(&"download_status"));

    let call = responses
        .iter()
        .find(|response| response["id"] == 3)
        .expect("tools/call response");
    assert_eq!(
        call["result"]["structuredContent"]["playback"]["volume"],
        50
    );
    assert_eq!(
        call["result"]["structuredContent"]["playback"]["current"]["video_id"],
        "v1"
    );
}
