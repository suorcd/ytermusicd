use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::{io, net::SocketAddr, time::Instant};
use tokio::{net::TcpListener, signal};

#[derive(Clone)]
pub struct AppState {
    started_at: Instant,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub status: &'static str,
    pub uptime_seconds: u64,
    /// Populated by the auth/core work in #6.
    pub auth: &'static str,
    /// Populated by the playback/queue service in #7.
    pub playback: Option<()>,
    pub queue_length: usize,
    pub downloads: DownloadSummary,
}

#[derive(Debug, Default, Serialize)]
pub struct DownloadSummary {
    pub queued: usize,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .with_state(state)
}

async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    log::debug!("GET /api/status");
    Json(StatusResponse {
        name: "ytermusicd",
        version: env!("CARGO_PKG_VERSION"),
        status: "ok",
        uptime_seconds: state.started_at.elapsed().as_secs(),
        auth: "unknown",
        playback: None,
        queue_length: 0,
        downloads: DownloadSummary::default(),
    })
}

pub async fn serve(bind: &str, port: u16) -> io::Result<()> {
    let addr: SocketAddr = format!("{bind}:{port}").parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid bind address {bind}:{port}: {error}"),
        )
    })?;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("failed to bind {addr}: {error}")))?;
    let local_addr = listener.local_addr()?;
    println!("ytermusicd listening on http://{local_addr}");

    axum::serve(listener, router(AppState::new()))
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    log::info!("shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn status_endpoint_returns_json() {
        let app = router(AppState::new());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let response = reqwest::get(format!("http://{addr}/api/status"))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value =
            serde_json::from_str(&response.text().await.unwrap()).unwrap();
        assert_eq!(body["name"], "ytermusicd");
        assert_eq!(body["status"], "ok");
        assert!(body["uptime_seconds"].is_u64());
        assert!(body["playback"].is_null());
        assert_eq!(body["queue_length"], 0);
        assert_eq!(body["downloads"]["queued"], 0);

        server.abort();
    }

    #[tokio::test]
    async fn rejects_invalid_bind_address() {
        let error = serve("not-an-ip", 8080).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
