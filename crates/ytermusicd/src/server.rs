use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::{io, net::SocketAddr, time::Instant};
use tokio::{net::TcpListener, signal};
use ytermusicd_core::{AuthStatus, PlaybackSnapshot, QueueSnapshot};

#[derive(Clone)]
pub struct AppState {
    started_at: Instant,
    auth: AuthStatus,
    playback: PlaybackSnapshot,
    queue: QueueSnapshot,
    downloads: DownloadSummary,
}

impl AppState {
    pub fn new(
        auth: AuthStatus,
        playback: PlaybackSnapshot,
        queue: QueueSnapshot,
        downloads: DownloadSummary,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            auth,
            playback,
            queue,
            downloads,
        }
    }

    pub fn with_defaults(auth: AuthStatus) -> Self {
        Self::new(
            auth,
            PlaybackSnapshot::default(),
            QueueSnapshot::default(),
            DownloadSummary::default(),
        )
    }
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub status: &'static str,
    pub uptime_seconds: u64,
    pub auth: AuthStatus,
    pub playback: PlaybackSnapshot,
    pub queue_length: usize,
    pub downloads: DownloadSummary,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
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
        auth: state.auth,
        playback: state.playback,
        queue_length: state.queue.items.len(),
        downloads: state.downloads,
    })
}

pub async fn serve(bind: &str, port: u16, auth: AuthStatus) -> io::Result<()> {
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

    axum::serve(listener, router(AppState::with_defaults(auth)))
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
    use common_structs::MusicDownloadStatus;
    use std::path::PathBuf;
    use ytermusicd_core::{AuthError, PlaybackStatus, QueueItem, RepeatMode, Track};

    async fn status_body(state: AppState) -> serde_json::Value {
        let app = router(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let response = reqwest::get(format!("http://{addr}/api/status"))
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = serde_json::from_str(&response.text().await.unwrap()).unwrap();
        server.abort();
        body
    }

    #[tokio::test]
    async fn status_endpoint_returns_json() {
        let body = status_body(AppState::with_defaults(AuthStatus::ok(PathBuf::from(
            "/tmp/headers.txt",
        ))))
        .await;
        assert_eq!(body["name"], "ytermusicd");
        assert_eq!(body["status"], "ok");
        assert_eq!(body["auth"]["state"], "ok");
        assert!(body["uptime_seconds"].is_u64());
        assert_eq!(body["playback"]["status"], "stopped");
        assert!(body["playback"]["current"].is_null());
        assert_eq!(body["queue_length"], 0);
        assert_eq!(body["downloads"]["queued"], 0);
    }

    #[tokio::test]
    async fn status_reports_playback_and_downloads() {
        let track = Track {
            video_id: "v1".to_string(),
            title: "Song".to_string(),
            author: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        };
        let playback = PlaybackSnapshot {
            status: PlaybackStatus::Playing,
            current_index: Some(0),
            current: Some(track.clone()),
            position_secs: 12.0,
            volume: 75,
            repeat: RepeatMode::All,
            shuffle: true,
        };
        let queue = QueueSnapshot {
            items: vec![QueueItem {
                track,
                download: MusicDownloadStatus::Downloaded,
            }],
            current: Some(0),
            repeat: RepeatMode::All,
            shuffle: true,
        };
        let downloads = DownloadSummary {
            queued: 1,
            active: 2,
            completed: 3,
            failed: 4,
        };
        let body = status_body(AppState::new(
            AuthStatus::ok(PathBuf::from("/tmp/headers.txt")),
            playback,
            queue,
            downloads,
        ))
        .await;

        assert_eq!(body["playback"]["status"], "playing");
        assert_eq!(body["playback"]["current"]["video_id"], "v1");
        assert_eq!(body["playback"]["volume"], 75);
        assert_eq!(body["queue_length"], 1);
        assert_eq!(body["downloads"]["completed"], 3);
    }

    #[tokio::test]
    async fn status_reports_missing_auth() {
        let error = AuthError::NotFound {
            path: PathBuf::from("/tmp/headers.txt"),
        };
        let body = status_body(AppState::with_defaults(AuthStatus::from_error(&error))).await;
        assert_eq!(body["auth"]["state"], "missing");
        assert!(
            body["auth"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn rejects_invalid_bind_address() {
        let error = serve(
            "not-an-ip",
            8080,
            AuthStatus::ok(PathBuf::from("/tmp/headers.txt")),
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
