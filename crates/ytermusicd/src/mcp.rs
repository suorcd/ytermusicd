use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fmt, time::Duration};
use ytermusicd_api::{
    DownloadSummary, ErrorEnvelope, PlayPlaylistRequest, PlayTrackRequest, Playlist,
    PlaylistDetail, Queue, QueueItem, QueueRequest, SearchResults, Status, Track,
};

const DEFAULT_URL: &str = "http://127.0.0.1:8080";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// HTTP client for the daemon. It holds no playback state.
#[derive(Debug, Clone)]
pub struct DaemonClient {
    base_url: String,
    http: reqwest::Client,
}

#[derive(Debug)]
pub struct DaemonError {
    code: String,
    message: String,
}

impl DaemonError {
    fn unavailable(base_url: &str, error: impl fmt::Display) -> Self {
        Self {
            code: "daemon_unavailable".to_string(),
            message: format!(
                "Cannot reach the daemon at {base_url}. Start it with: ytermusicd serve ({error})"
            ),
        }
    }

    fn response(status: reqwest::StatusCode, envelope: Option<ErrorEnvelope>) -> Self {
        match envelope {
            Some(envelope) => Self {
                code: envelope.error.code,
                message: envelope.error.message,
            },
            None => Self {
                code: "http_error".to_string(),
                message: format!("the daemon returned HTTP {status}"),
            },
        }
    }

    fn invalid_response(error: impl fmt::Display) -> Self {
        Self {
            code: "invalid_response".to_string(),
            message: format!("the daemon returned an invalid response: {error}"),
        }
    }

    fn to_mcp(&self) -> McpError {
        McpError::internal_error(
            format!("daemon error ({}): {}", self.code, self.message),
            None,
        )
    }
}

impl fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DaemonError {}

impl DaemonClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, DaemonError> {
        self.request(reqwest::Method::GET, path, None).await
    }

    async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, DaemonError> {
        self.request(
            reqwest::Method::POST,
            path,
            Some(serde_json::to_value(body).map_err(DaemonError::invalid_response)?),
        )
        .await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, DaemonError> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base_url));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|error| DaemonError::unavailable(&self.base_url, error))?;
        let status = response.status();
        if !status.is_success() {
            let envelope = response.json::<ErrorEnvelope>().await.ok();
            return Err(DaemonError::response(status, envelope));
        }
        response
            .json::<T>()
            .await
            .map_err(DaemonError::invalid_response)
    }

    pub async fn status(&self) -> Result<Status, DaemonError> {
        self.get("/api/status").await
    }

    pub async fn playlists(&self) -> Result<Vec<Playlist>, DaemonError> {
        self.get("/api/playlists").await
    }

    pub async fn playlist(&self, playlist_id: &str) -> Result<PlaylistDetail, DaemonError> {
        self.get(&format!("/api/playlists/{}", encode(playlist_id)))
            .await
    }

    pub async fn search(&self, query: &str) -> Result<SearchResults, DaemonError> {
        self.get(&format!("/api/search?q={}", encode(query))).await
    }

    pub async fn queue(&self) -> Result<Queue, DaemonError> {
        self.get("/api/queue").await
    }

    pub async fn play_playlist(&self, playlist_id: &str) -> Result<Status, DaemonError> {
        self.post(
            "/api/play/playlist",
            &PlayPlaylistRequest {
                playlist_id: playlist_id.to_string(),
            },
        )
        .await
    }

    pub async fn play_track(&self, track: Track) -> Result<Status, DaemonError> {
        self.post("/api/play/track", &PlayTrackRequest { track })
            .await
    }

    pub async fn queue_add(&self, track: Track) -> Result<Queue, DaemonError> {
        self.post("/api/queue", &QueueRequest { track }).await
    }

    pub async fn pause(&self) -> Result<Status, DaemonError> {
        self.post("/api/pause", &serde_json::json!({})).await
    }

    pub async fn resume(&self) -> Result<Status, DaemonError> {
        self.post("/api/resume", &serde_json::json!({})).await
    }

    pub async fn next(&self) -> Result<Status, DaemonError> {
        self.post("/api/next", &serde_json::json!({})).await
    }

    pub async fn previous(&self) -> Result<Status, DaemonError> {
        self.post("/api/previous", &serde_json::json!({})).await
    }

    pub async fn volume(&self, volume: u8) -> Result<Status, DaemonError> {
        self.post("/api/volume", &serde_json::json!({ "volume": volume }))
            .await
    }
}

fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[derive(Debug, Clone)]
pub struct McpServer {
    client: DaemonClient,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new(client: DaemonClient) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    /// Text to search for.
    pub query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlaylistArgs {
    /// Playlist ID from list_playlists or search.
    pub playlist_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TrackArgs {
    /// Track object from search or get_playlist.
    pub track: Track,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VolumeArgs {
    /// Volume from 0 to 100.
    pub volume: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DownloadStatusArgs {
    /// Optional video ID. Omit it to list all queue downloads.
    pub video_id: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DownloadStatusOutput {
    pub summary: DownloadSummary,
    pub tracks: Vec<QueueItem>,
}

#[tool_router]
impl McpServer {
    /// Search YouTube Music for tracks and playlists.
    #[tool(description = "Search YouTube Music for tracks and playlists")]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<SearchResults>, McpError> {
        self.client
            .search(&args.query)
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// List the playlists in the YouTube Music library.
    #[tool(description = "List the playlists in the YouTube Music library")]
    async fn list_playlists(&self) -> Result<Json<Vec<Playlist>>, McpError> {
        self.client
            .playlists()
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Get one playlist and its tracks.
    #[tool(description = "Get one playlist and its tracks")]
    async fn get_playlist(
        &self,
        Parameters(args): Parameters<PlaylistArgs>,
    ) -> Result<Json<PlaylistDetail>, McpError> {
        self.client
            .playlist(&args.playlist_id)
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Replace the queue with a playlist and start playback.
    #[tool(description = "Replace the queue with a playlist and start playback")]
    async fn play_playlist(
        &self,
        Parameters(args): Parameters<PlaylistArgs>,
    ) -> Result<Json<Status>, McpError> {
        self.client
            .play_playlist(&args.playlist_id)
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Play a track immediately.
    #[tool(description = "Play a track immediately")]
    async fn play_track(
        &self,
        Parameters(args): Parameters<TrackArgs>,
    ) -> Result<Json<Status>, McpError> {
        self.client
            .play_track(args.track)
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Add a track to the queue.
    #[tool(description = "Add a track to the queue")]
    async fn queue_add(
        &self,
        Parameters(args): Parameters<TrackArgs>,
    ) -> Result<Json<Queue>, McpError> {
        self.client
            .queue_add(args.track)
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Get the current track and playback state.
    #[tool(description = "Get the current track and playback state")]
    async fn now_playing(&self) -> Result<Json<Status>, McpError> {
        self.client
            .status()
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Pause playback.
    #[tool(description = "Pause playback")]
    async fn pause(&self) -> Result<Json<Status>, McpError> {
        self.client
            .pause()
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Resume playback.
    #[tool(description = "Resume playback")]
    async fn resume(&self) -> Result<Json<Status>, McpError> {
        self.client
            .resume()
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Skip to the next track.
    #[tool(description = "Skip to the next track")]
    async fn next(&self) -> Result<Json<Status>, McpError> {
        self.client
            .next()
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Go to the previous track.
    #[tool(description = "Go to the previous track")]
    async fn previous(&self) -> Result<Json<Status>, McpError> {
        self.client
            .previous()
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Set the playback volume.
    #[tool(description = "Set the playback volume (0-100)")]
    async fn volume(
        &self,
        Parameters(args): Parameters<VolumeArgs>,
    ) -> Result<Json<Status>, McpError> {
        if args.volume > 100 {
            return Err(McpError::invalid_params(
                "volume must be between 0 and 100",
                None,
            ));
        }
        self.client
            .volume(args.volume)
            .await
            .map(Json)
            .map_err(|error| error.to_mcp())
    }

    /// Report download status for queue tracks.
    #[tool(description = "Report download status for queue tracks")]
    async fn download_status(
        &self,
        Parameters(args): Parameters<DownloadStatusArgs>,
    ) -> Result<Json<DownloadStatusOutput>, McpError> {
        let (status, queue) = tokio::join!(self.client.status(), self.client.queue());
        let status = status.map_err(|error| error.to_mcp())?;
        let queue = queue.map_err(|error| error.to_mcp())?;
        let tracks = match args.video_id {
            Some(video_id) => {
                let item = queue
                    .items
                    .into_iter()
                    .find(|item| item.track.video_id == video_id);
                match item {
                    Some(item) => vec![item],
                    None => {
                        return Err(McpError::invalid_params(
                            format!("no queued track with video id {video_id}"),
                            None,
                        ));
                    }
                }
            }
            None => queue.items,
        };
        Ok(Json(DownloadStatusOutput {
            summary: status.downloads,
            tracks,
        }))
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Tools for the ytermusicd daemon. Playlist editing, lyrics and HTTP transport are post-v1.",
        )
    }
}

/// Run the stdio MCP proxy. It exits when stdin closes.
pub async fn run(base_url: Option<String>) -> std::io::Result<()> {
    let base_url = base_url.unwrap_or_else(|| DEFAULT_URL.to_string());
    let client = DaemonClient::new(base_url).map_err(std::io::Error::other)?;
    let server = McpServer::new(client);
    log::info!(
        "MCP server ready with tools: {}",
        server.tool_names().join(", ")
    );
    let service = server.serve(stdio()).await.map_err(std::io::Error::other)?;
    service.waiting().await.map_err(std::io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json as AxumJson, Router,
        extract::{Path, State},
        http::StatusCode,
        routing::{get, post},
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use ytermusicd_api::{
        AuthState, AuthStatus, DownloadStatus, PlaybackSnapshot, PlaybackState, Playlist,
        QueueItem, RepeatMode,
    };

    #[derive(Clone, Default)]
    struct MockState {
        calls: Arc<Mutex<Vec<String>>>,
        fail: Arc<AtomicBool>,
    }

    fn track(id: &str) -> Track {
        Track {
            video_id: id.to_string(),
            title: format!("Track {id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        }
    }

    fn status() -> Status {
        Status {
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
                current: Some(track("v1")),
                position_secs: 12.0,
                volume: 50,
                repeat: RepeatMode::Off,
                shuffle: false,
            },
            queue_length: 1,
            downloads: DownloadSummary {
                queued: 1,
                active: 0,
                completed: 1,
                retrying: 0,
                failed: 0,
            },
        }
    }

    fn queue() -> Queue {
        Queue {
            items: vec![QueueItem {
                track: track("v1"),
                download: DownloadStatus::Downloaded,
            }],
            current: Some(0),
            repeat: RepeatMode::Off,
            shuffle: false,
        }
    }

    async fn mock_router(state: MockState) -> (String, tokio::task::JoinHandle<()>) {
        let router = Router::new()
            .route("/api/status", get(mock_status))
            .route("/api/playlists", get(mock_playlists))
            .route("/api/playlists/{id}", get(mock_playlist))
            .route("/api/search", get(mock_search))
            .route("/api/queue", get(mock_queue).post(mock_queue_add))
            .route("/api/play/playlist", post(mock_play_playlist))
            .route("/api/play/track", post(mock_play_track))
            .route("/api/pause", post(mock_status_post))
            .route("/api/resume", post(mock_status_post))
            .route("/api/next", post(mock_status_post))
            .route("/api/previous", post(mock_status_post))
            .route("/api/volume", post(mock_volume))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{addr}"), handle)
    }

    async fn mock_status(State(state): State<MockState>) -> Result<AxumJson<Status>, StatusCode> {
        state
            .calls
            .lock()
            .unwrap()
            .push("GET /api/status".to_string());
        if state.fail.load(Ordering::SeqCst) {
            return Err(StatusCode::BAD_GATEWAY);
        }
        Ok(AxumJson(status()))
    }

    async fn mock_playlists(State(state): State<MockState>) -> AxumJson<Vec<Playlist>> {
        state
            .calls
            .lock()
            .unwrap()
            .push("GET /api/playlists".to_string());
        AxumJson(vec![Playlist {
            id: "p1".to_string(),
            name: "Playlist One".to_string(),
            subtitle: "Artist".to_string(),
        }])
    }

    async fn mock_playlist(
        State(state): State<MockState>,
        Path(id): Path<String>,
    ) -> AxumJson<PlaylistDetail> {
        state
            .calls
            .lock()
            .unwrap()
            .push(format!("GET /api/playlists/{id}"));
        AxumJson(PlaylistDetail {
            playlist: Playlist {
                id: id.clone(),
                name: "Playlist One".to_string(),
                subtitle: "Artist".to_string(),
            },
            tracks: vec![track("v1")],
        })
    }

    async fn mock_search(State(state): State<MockState>) -> AxumJson<SearchResults> {
        state
            .calls
            .lock()
            .unwrap()
            .push("GET /api/search".to_string());
        AxumJson(SearchResults {
            tracks: vec![track("v1")],
            playlists: vec![],
        })
    }

    async fn mock_queue(State(state): State<MockState>) -> AxumJson<Queue> {
        state
            .calls
            .lock()
            .unwrap()
            .push("GET /api/queue".to_string());
        AxumJson(queue())
    }

    async fn mock_play_playlist(
        State(state): State<MockState>,
        AxumJson(_request): AxumJson<PlayPlaylistRequest>,
    ) -> AxumJson<Status> {
        state
            .calls
            .lock()
            .unwrap()
            .push("POST /api/play/playlist".to_string());
        AxumJson(status())
    }

    async fn mock_play_track(
        State(state): State<MockState>,
        AxumJson(_request): AxumJson<PlayTrackRequest>,
    ) -> AxumJson<Status> {
        state
            .calls
            .lock()
            .unwrap()
            .push("POST /api/play/track".to_string());
        AxumJson(status())
    }

    async fn mock_queue_add(
        State(state): State<MockState>,
        AxumJson(_request): AxumJson<QueueRequest>,
    ) -> AxumJson<Queue> {
        state
            .calls
            .lock()
            .unwrap()
            .push("POST /api/queue".to_string());
        AxumJson(queue())
    }

    async fn mock_status_post(State(state): State<MockState>) -> AxumJson<Status> {
        state
            .calls
            .lock()
            .unwrap()
            .push("POST transport".to_string());
        AxumJson(status())
    }

    async fn mock_volume(
        State(state): State<MockState>,
        AxumJson(request): AxumJson<serde_json::Value>,
    ) -> AxumJson<Status> {
        state
            .calls
            .lock()
            .unwrap()
            .push(format!("POST /api/volume {}", request["volume"]));
        AxumJson(status())
    }

    async fn server() -> (McpServer, MockState, tokio::task::JoinHandle<()>) {
        let state = MockState::default();
        let (base, handle) = mock_router(state.clone()).await;
        (
            McpServer::new(DaemonClient::new(base).unwrap()),
            state,
            handle,
        )
    }

    fn expect_error<T>(result: Result<T, McpError>) -> McpError {
        match result {
            Ok(_) => panic!("expected an MCP error"),
            Err(error) => error,
        }
    }

    #[test]
    fn lists_the_13_v1_tools() {
        let server = McpServer::new(DaemonClient::new("http://127.0.0.1:1").unwrap());
        let mut names = server.tool_names();
        names.sort();
        assert_eq!(
            names,
            vec![
                "download_status",
                "get_playlist",
                "list_playlists",
                "next",
                "now_playing",
                "pause",
                "play_playlist",
                "play_track",
                "previous",
                "queue_add",
                "resume",
                "search",
                "volume",
            ]
        );
    }

    #[tokio::test]
    async fn tools_call_the_daemon_and_return_structured_data() {
        let (server, state, handle) = server().await;
        let track = track("v1");

        let Json(found) = server
            .search(Parameters(SearchArgs {
                query: "test".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(found.tracks[0].video_id, "v1");

        let Json(playlists) = server.list_playlists().await.unwrap();
        assert_eq!(playlists[0].id, "p1");

        let Json(detail) = server
            .get_playlist(Parameters(PlaylistArgs {
                playlist_id: "p1".to_string(),
            }))
            .await
            .unwrap();
        assert_eq!(detail.tracks.len(), 1);

        let Json(played) = server
            .play_track(Parameters(TrackArgs {
                track: track.clone(),
            }))
            .await
            .unwrap();
        assert_eq!(played.playback.state, PlaybackState::Playing);

        let Json(queued) = server
            .queue_add(Parameters(TrackArgs { track }))
            .await
            .unwrap();
        assert_eq!(queued.items.len(), 1);

        let Json(now) = server.now_playing().await.unwrap();
        assert_eq!(now.playback.volume, 50);

        let Json(volume) = server
            .volume(Parameters(VolumeArgs { volume: 75 }))
            .await
            .unwrap();
        assert_eq!(volume.playback.volume, 50);

        let Json(downloads) = server
            .download_status(Parameters(DownloadStatusArgs {
                video_id: Some("v1".to_string()),
            }))
            .await
            .unwrap();
        assert_eq!(downloads.tracks.len(), 1);
        assert_eq!(downloads.summary.completed, 1);

        let calls = state.calls.lock().unwrap().clone();
        assert!(calls.contains(&"GET /api/search".to_string()));
        assert!(calls.contains(&"POST /api/play/track".to_string()));
        assert!(calls.contains(&"POST /api/volume 75".to_string()));
        handle.abort();
    }

    #[tokio::test]
    async fn command_tools_cover_transport() {
        let (server, _state, handle) = server().await;
        assert_eq!(
            server.pause().await.unwrap().0.playback.state,
            PlaybackState::Playing
        );
        assert_eq!(
            server.resume().await.unwrap().0.playback.state,
            PlaybackState::Playing
        );
        assert_eq!(
            server.next().await.unwrap().0.playback.state,
            PlaybackState::Playing
        );
        assert_eq!(
            server.previous().await.unwrap().0.playback.state,
            PlaybackState::Playing
        );
        assert_eq!(
            server
                .play_playlist(Parameters(PlaylistArgs {
                    playlist_id: "p1".to_string(),
                }))
                .await
                .unwrap()
                .0
                .playback
                .state,
            PlaybackState::Playing
        );
        handle.abort();
    }

    #[tokio::test]
    async fn rejects_out_of_range_volume() {
        let (server, _state, handle) = server().await;
        let error = expect_error(server.volume(Parameters(VolumeArgs { volume: 101 })).await);
        assert!(error.message.contains("0 and 100"));
        handle.abort();
    }

    #[tokio::test]
    async fn unavailable_daemon_is_actionable() {
        let client = DaemonClient::new("http://127.0.0.1:1").unwrap();
        let server = McpServer::new(client);
        let error = expect_error(server.now_playing().await);
        assert!(error.message.contains("Cannot reach the daemon"));
        assert!(error.message.contains("ytermusicd serve"));
    }

    #[tokio::test]
    async fn upstream_errors_keep_the_daemon_message() {
        let state = MockState::default();
        state.fail.store(true, Ordering::SeqCst);
        let (base, handle) = mock_router(state).await;
        let server = McpServer::new(DaemonClient::new(base).unwrap());
        let error = expect_error(server.now_playing().await);
        assert!(error.message.contains("HTTP 502"));
        handle.abort();
    }

    #[test]
    fn encodes_paths_and_queries() {
        assert_eq!(encode("VL/1 & x"), "VL%2F1%20%26%20x");
    }
}
