use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::Deserialize;
use std::{
    convert::Infallible,
    io,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    signal,
    sync::{Mutex, mpsc::UnboundedSender},
};
use ytermusicd_api::{
    ApiError, AuthState, AuthStatus as ApiAuthStatus, DownloadStatus, DownloadSummary,
    ErrorEnvelope, Event, EventKind, PlayNextRequest, PlayPlaylistRequest, PlayTrackRequest,
    PlaybackSnapshot as ApiPlaybackSnapshot, PlaybackState, Playlist, PlaylistDetail,
    Queue as ApiQueue, QueueItem as ApiQueueItem, QueueRequest, RemoveQueueRequest,
    ReorderQueueRequest, RepeatMode as ApiRepeatMode, RepeatRequest, SearchResults, SeekRequest,
    ShuffleRequest, Status, Track, VolumeRequest,
};
use ytermusicd_core::{
    AudioSink, AuthState as CoreAuthState, AuthStatus, CoreEvent, EventBus, PlaybackCommand,
    PlaybackError, PlaybackService, PlaybackSnapshot, PlaybackStatus, QueueSnapshot,
    RepeatMode as CoreRepeatMode,
};

use crate::music::{MusicApi, core_track};

pub struct AppState<S: AudioSink + Send + 'static> {
    started_at: Instant,
    auth: AuthStatus,
    music: Arc<dyn MusicApi>,
    playback: Arc<Mutex<PlaybackService<S>>>,
    events: EventBus,
    downloads: DownloadSummary,
}

impl<S: AudioSink + Send + 'static> Clone for AppState<S> {
    fn clone(&self) -> Self {
        Self {
            started_at: self.started_at,
            auth: self.auth.clone(),
            music: self.music.clone(),
            playback: self.playback.clone(),
            events: self.events.clone(),
            downloads: self.downloads,
        }
    }
}

impl<S: AudioSink + Send + 'static> AppState<S> {
    pub fn new(
        auth: AuthStatus,
        music: Arc<dyn MusicApi>,
        playback: Arc<Mutex<PlaybackService<S>>>,
        events: EventBus,
        downloads: DownloadSummary,
    ) -> Self {
        Self {
            started_at: Instant::now(),
            auth,
            music,
            playback,
            events,
            downloads,
        }
    }
}

#[derive(Debug)]
struct ApiFailure {
    status: StatusCode,
    error: ApiError,
}

impl ApiFailure {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error: ApiError {
                code: "bad_request".to_string(),
                message: message.into(),
            },
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error: ApiError {
                code: "not_found".to_string(),
                message: message.into(),
            },
        }
    }

    fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            error: ApiError {
                code: "upstream_error".to_string(),
                message: message.into(),
            },
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: ApiError {
                code: "internal_error".to_string(),
                message: message.into(),
            },
        }
    }
}

impl IntoResponse for ApiFailure {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorEnvelope { error: self.error })).into_response()
    }
}

pub fn router<S: AudioSink + Send + 'static>(state: AppState<S>) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/playlists", get(playlists))
        .route("/api/playlists/{id}", get(playlist))
        .route("/api/search", get(search))
        .route("/api/queue", get(queue).post(queue_add).delete(queue_clear))
        .route("/api/queue/reorder", post(queue_reorder))
        .route("/api/queue/remove", post(queue_remove))
        .route("/api/queue/play-next", post(queue_play_next))
        .route("/api/play/playlist", post(play_playlist))
        .route("/api/play/track", post(play_track))
        .route("/api/queue/add", post(queue_add))
        .route("/api/pause", post(pause))
        .route("/api/resume", post(resume))
        .route("/api/next", post(next))
        .route("/api/previous", post(previous))
        .route("/api/volume", post(volume))
        .route("/api/seek", post(seek))
        .route("/api/shuffle", post(shuffle))
        .route("/api/repeat", post(repeat))
        .route("/api/events", get(events))
        .with_state(state)
}

async fn status<S: AudioSink + Send + 'static>(State(state): State<AppState<S>>) -> Json<Status> {
    let playback = state.playback.lock().await;
    Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    ))
}

async fn playlists<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
) -> Result<Json<Vec<Playlist>>, ApiFailure> {
    state
        .music
        .playlists()
        .await
        .map(Json)
        .map_err(ApiFailure::upstream)
}

async fn playlist<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<PlaylistDetail>, ApiFailure> {
    let playlist = state
        .music
        .playlists()
        .await
        .map_err(ApiFailure::upstream)?
        .into_iter()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| ApiFailure::not_found(format!("playlist {id} not found")))?;
    let tracks = state
        .music
        .playlist(&id)
        .await
        .map_err(ApiFailure::upstream)?;
    Ok(Json(PlaylistDetail { playlist, tracks }))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResults>, ApiFailure> {
    if query.q.trim().is_empty() {
        return Err(ApiFailure::bad_request("search query must not be empty"));
    }
    state
        .music
        .search(&query.q)
        .await
        .map(Json)
        .map_err(ApiFailure::upstream)
}

async fn queue<S: AudioSink + Send + 'static>(State(state): State<AppState<S>>) -> Json<ApiQueue> {
    let playback = state.playback.lock().await;
    Json(api_queue(&playback.queue_snapshot()))
}

async fn play_playlist<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<PlayPlaylistRequest>,
) -> Result<Json<Status>, ApiFailure> {
    let tracks = state
        .music
        .playlist(&request.playlist_id)
        .await
        .map_err(ApiFailure::upstream)?;
    if tracks.is_empty() {
        return Err(ApiFailure::bad_request("playlist has no tracks"));
    }
    let mut playback = state.playback.lock().await;
    playback
        .play_playlist(tracks.into_iter().map(core_track).collect())
        .map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn play_track<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<PlayTrackRequest>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback
        .play_track(core_track(request.track))
        .map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn queue_add<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<QueueRequest>,
) -> Result<Json<ApiQueue>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.enqueue(core_track(request.track));
    Ok(Json(api_queue(&playback.queue_snapshot())))
}

async fn pause<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.pause().map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn resume<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.resume().map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn next<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.next_track().map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn previous<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.previous_track().map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn volume<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<VolumeRequest>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback
        .set_volume(request.volume)
        .map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn seek<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<SeekRequest>,
) -> Result<Json<Status>, ApiFailure> {
    if request.position_secs < 0.0 {
        return Err(ApiFailure::bad_request("position must be >= 0"));
    }
    let mut playback = state.playback.lock().await;
    playback
        .seek(Duration::from_secs_f64(request.position_secs))
        .map_err(playback_failure)?;
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn shuffle<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<ShuffleRequest>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.set_shuffle(request.enabled);
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn repeat<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<RepeatRequest>,
) -> Result<Json<Status>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.set_repeat(core_repeat(request.repeat));
    Ok(Json(status_from_parts(
        &state,
        &playback.snapshot(),
        &playback.queue_snapshot(),
    )))
}

async fn queue_clear<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
) -> Result<Json<ApiQueue>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    playback.clear().map_err(playback_failure)?;
    Ok(Json(api_queue(&playback.queue_snapshot())))
}

async fn queue_reorder<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<ReorderQueueRequest>,
) -> Result<Json<ApiQueue>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    if !playback.move_item(request.from, request.to) {
        return Err(ApiFailure::bad_request("invalid queue indices"));
    }
    Ok(Json(api_queue(&playback.queue_snapshot())))
}

async fn queue_remove<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<RemoveQueueRequest>,
) -> Result<Json<ApiQueue>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    if request.index >= playback.queue_snapshot().items.len() {
        return Err(ApiFailure::bad_request("invalid queue index"));
    }
    playback.remove(request.index).map_err(playback_failure)?;
    Ok(Json(api_queue(&playback.queue_snapshot())))
}

async fn queue_play_next<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    Json(request): Json<PlayNextRequest>,
) -> Result<Json<ApiQueue>, ApiFailure> {
    let mut playback = state.playback.lock().await;
    let current = playback.queue_snapshot().current.unwrap_or(0);
    let target = (current + 1).min(playback.queue_snapshot().items.len().saturating_sub(1));
    if !playback.move_item(request.index, target) {
        return Err(ApiFailure::bad_request("invalid queue index"));
    }
    Ok(Json(api_queue(&playback.queue_snapshot())))
}

async fn events<S: AudioSink + Send + 'static>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let subscription = state.events.subscribe_since(last_event_id);
    let snapshot_status = status_from_parts(
        &state,
        &subscription.snapshot.playback,
        &subscription.snapshot.queue,
    );
    let snapshot_queue = api_queue(&subscription.snapshot.queue);

    let stream = async_stream::stream! {
        let mut last_sent = if subscription.needs_snapshot {
            let snapshot = Event {
                id: subscription.latest_id,
                event: EventKind::Snapshot {
                    status: Box::new(snapshot_status),
                    queue: snapshot_queue,
                },
            };
            yield Ok::<SseEvent, Infallible>(SseEvent::default()
                .event("snapshot")
                .id(subscription.latest_id.to_string())
                .json_data(snapshot)
                .expect("snapshot serializes"));
            subscription.latest_id
        } else {
            last_event_id.unwrap_or(0)
        };

        for envelope in subscription.replay {
            last_sent = envelope.id;
            if let Some(event) = api_event(envelope.event) {
                yield Ok::<SseEvent, Infallible>(SseEvent::default()
                    .id(envelope.id.to_string())
                    .json_data(event)
                    .expect("event serializes"));
            }
        }

        let mut receiver = subscription.receiver;
        loop {
            match receiver.recv().await {
                Ok(envelope) => {
                    // The receiver is subscribed before history is read, so an
                    // event published in that window can be replayed twice.
                    if envelope.id <= last_sent {
                        continue;
                    }
                    last_sent = envelope.id;
                    if let Some(event) = api_event(envelope.event) {
                        yield Ok::<SseEvent, Infallible>(SseEvent::default()
                            .id(envelope.id.to_string())
                            .json_data(event)
                            .expect("event serializes"));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    log::warn!("SSE client lagged by {count} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    let sse = Sse::new(stream).keep_alive(KeepAlive::default());
    (
        [(
            axum::http::header::HeaderName::from_static("x-accel-buffering"),
            "no",
        )],
        sse,
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn serve<S: AudioSink + Send + 'static>(
    bind: &str,
    port: u16,
    auth: AuthStatus,
    music: Arc<dyn MusicApi>,
    playback: Arc<Mutex<PlaybackService<S>>>,
    downloads: DownloadSummary,
    commands: UnboundedSender<PlaybackCommand>,
    dbus: bool,
) -> io::Result<()> {
    let addr: SocketAddr = format!("{bind}:{port}").parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid bind address {bind}:{port}: {error}"),
        )
    })?;

    let events = playback.lock().await.events();
    if dbus {
        match crate::mpris::spawn(commands, events.clone()) {
            Ok(_) => log::info!("MPRIS media controls enabled"),
            Err(error) => log::warn!("MPRIS unavailable: {error}"),
        }
    }

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|error| io::Error::new(error.kind(), format!("failed to bind {addr}: {error}")))?;
    let local_addr = listener.local_addr()?;
    println!("ytermusicd listening on http://{local_addr}");

    let state = AppState::new(auth, music, playback, events, downloads);
    axum::serve(listener, router(state))
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

fn status_from_parts<S: AudioSink + Send + 'static>(
    state: &AppState<S>,
    playback: &PlaybackSnapshot,
    queue: &QueueSnapshot,
) -> Status {
    Status {
        name: "ytermusicd".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "ok".to_string(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
        auth: api_auth(&state.auth),
        playback: api_playback(playback),
        queue_length: queue.items.len(),
        downloads: state.downloads,
    }
}

fn api_auth(auth: &AuthStatus) -> ApiAuthStatus {
    ApiAuthStatus {
        state: match auth.state {
            CoreAuthState::Ok => AuthState::Ok,
            CoreAuthState::Missing => AuthState::Missing,
            CoreAuthState::Invalid => AuthState::Invalid,
        },
        source: auth.source.as_ref().map(|path| path.display().to_string()),
        message: auth.message.clone(),
    }
}

fn api_playback(snapshot: &PlaybackSnapshot) -> ApiPlaybackSnapshot {
    ApiPlaybackSnapshot {
        state: match snapshot.status {
            PlaybackStatus::Stopped => PlaybackState::Stopped,
            PlaybackStatus::Playing => PlaybackState::Playing,
            PlaybackStatus::Paused => PlaybackState::Paused,
        },
        current: snapshot.current.as_ref().map(api_track),
        position_secs: snapshot.position_secs,
        volume: snapshot.volume,
        repeat: api_repeat(snapshot.repeat),
        shuffle: snapshot.shuffle,
    }
}

fn api_queue(queue: &QueueSnapshot) -> ApiQueue {
    ApiQueue {
        items: queue
            .items
            .iter()
            .map(|item| ApiQueueItem {
                track: api_track(&item.track),
                download: api_download(item.download),
            })
            .collect(),
        current: queue.current,
        repeat: api_repeat(queue.repeat),
        shuffle: queue.shuffle,
    }
}

fn api_track(track: &ytermusicd_core::Track) -> Track {
    Track {
        video_id: track.video_id.clone(),
        title: track.title.clone(),
        artist: track.author.clone(),
        album: track.album.clone(),
        duration: track.duration.clone(),
    }
}

fn api_download(status: common_structs::MusicDownloadStatus) -> DownloadStatus {
    match status {
        common_structs::MusicDownloadStatus::NotDownloaded => DownloadStatus::NotDownloaded,
        common_structs::MusicDownloadStatus::Downloaded => DownloadStatus::Downloaded,
        common_structs::MusicDownloadStatus::Downloading(progress) => DownloadStatus::Downloading {
            progress: progress.min(100) as u8,
        },
        common_structs::MusicDownloadStatus::Retrying(attempt) => {
            DownloadStatus::Retrying { attempt }
        }
        common_structs::MusicDownloadStatus::DownloadFailed => {
            DownloadStatus::Failed { message: None }
        }
    }
}

fn api_repeat(repeat: CoreRepeatMode) -> ApiRepeatMode {
    match repeat {
        CoreRepeatMode::Off => ApiRepeatMode::Off,
        CoreRepeatMode::One => ApiRepeatMode::One,
        CoreRepeatMode::All => ApiRepeatMode::All,
    }
}

fn core_repeat(repeat: ApiRepeatMode) -> CoreRepeatMode {
    match repeat {
        ApiRepeatMode::Off => CoreRepeatMode::Off,
        ApiRepeatMode::One => CoreRepeatMode::One,
        ApiRepeatMode::All => CoreRepeatMode::All,
    }
}

fn api_event(event: CoreEvent) -> Option<EventKind> {
    match event {
        CoreEvent::Playback(snapshot) => Some(EventKind::Playback {
            playback: api_playback(&snapshot),
        }),
        CoreEvent::Queue(queue) => Some(EventKind::Queue {
            queue: api_queue(&queue),
        }),
        CoreEvent::DownloadStatus { video_id, status } => Some(EventKind::Download {
            video_id,
            status: api_download(status),
        }),
        CoreEvent::TrackEnded { video_id } => Some(EventKind::TrackEnded { video_id }),
        CoreEvent::Error { message } => Some(EventKind::Error {
            error: ApiError {
                code: "core_error".to_string(),
                message,
            },
        }),
        CoreEvent::LibraryChanged => Some(EventKind::LibraryChanged),
    }
}

fn playback_failure(error: PlaybackError) -> ApiFailure {
    match error {
        PlaybackError::NoCurrentTrack => ApiFailure::bad_request("no current track"),
        PlaybackError::IndexOutOfBounds => ApiFailure::bad_request("queue index out of bounds"),
        PlaybackError::Sink(message) => ApiFailure::internal(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_util::StreamExt;
    use std::path::Path;
    use ytermusicd_core::Track as CoreTrack;

    #[derive(Default)]
    struct MockSink;

    impl AudioSink for MockSink {
        type Error = std::io::Error;

        fn play(&mut self, _track: &CoreTrack, _path: &Path) -> Result<(), Self::Error> {
            Ok(())
        }
        fn pause(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn seek(&mut self, _position: Duration) -> Result<(), Self::Error> {
            Ok(())
        }
        fn set_volume(&mut self, _volume: u8) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct MockMusicApi;

    fn track(id: &str) -> Track {
        Track {
            video_id: id.to_string(),
            title: format!("Track {id}"),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        }
    }

    #[async_trait]
    impl MusicApi for MockMusicApi {
        async fn playlists(&self) -> Result<Vec<Playlist>, String> {
            Ok(vec![Playlist {
                id: "p1".to_string(),
                name: "Playlist One".to_string(),
                subtitle: "Artist".to_string(),
            }])
        }

        async fn playlist(&self, _id: &str) -> Result<Vec<Track>, String> {
            Ok(vec![track("v1"), track("v2")])
        }

        async fn search(&self, _query: &str) -> Result<SearchResults, String> {
            Ok(SearchResults {
                tracks: vec![track("v3")],
                playlists: vec![],
            })
        }
    }

    struct FailingMusicApi;

    #[async_trait]
    impl MusicApi for FailingMusicApi {
        async fn playlists(&self) -> Result<Vec<Playlist>, String> {
            Err("upstream unavailable".to_string())
        }

        async fn playlist(&self, _id: &str) -> Result<Vec<Track>, String> {
            Err("upstream unavailable".to_string())
        }

        async fn search(&self, _query: &str) -> Result<SearchResults, String> {
            Err("upstream unavailable".to_string())
        }
    }

    fn test_state() -> AppState<MockSink> {
        test_state_with(Arc::new(MockMusicApi))
    }

    fn test_state_with(music: Arc<dyn MusicApi>) -> AppState<MockSink> {
        let service = PlaybackService::new(MockSink, "/tmp/ytermusicd-test");
        let events = service.events();
        let playback = Arc::new(Mutex::new(service));
        AppState::new(
            AuthStatus::ok("/tmp/headers.txt"),
            music,
            playback,
            events,
            DownloadSummary::default(),
        )
    }

    async fn read_until<B, S>(stream: &mut S, buffer: &mut String, needle: &str)
    where
        B: AsRef<[u8]>,
        S: futures_util::Stream<Item = Result<B, reqwest::Error>> + Unpin,
    {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !buffer.contains(needle) {
            let chunk = tokio::time::timeout_at(deadline, stream.next())
                .await
                .expect("timed out waiting for SSE data")
                .expect("SSE stream ended")
                .expect("SSE stream error");
            buffer.push_str(&String::from_utf8_lossy(chunk.as_ref()));
        }
    }

    fn last_event_id(buffer: &str) -> u64 {
        buffer
            .lines()
            .filter_map(|line| line.strip_prefix("id:"))
            .filter_map(|id| id.trim().parse::<u64>().ok())
            .max()
            .expect("at least one SSE event id")
    }

    async fn start_server(state: AppState<MockSink>) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router(state)).await.unwrap();
        });
        (format!("http://{addr}"), server)
    }

    #[tokio::test]
    async fn status_and_library_endpoints() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();

        let status: serde_json::Value = client
            .get(format!("{base}/api/status"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(status["auth"]["state"], "ok");
        assert_eq!(status["playback"]["state"], "stopped");

        let playlists: serde_json::Value = client
            .get(format!("{base}/api/playlists"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(playlists[0]["id"], "p1");

        let detail: serde_json::Value = client
            .get(format!("{base}/api/playlists/p1"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(detail["tracks"].as_array().unwrap().len(), 2);

        let search: serde_json::Value = client
            .get(format!("{base}/api/search?q=test"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(search["tracks"][0]["video_id"], "v3");
        server.abort();
    }

    #[tokio::test]
    async fn playback_and_queue_commands() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();

        let started: serde_json::Value = client
            .post(format!("{base}/api/play/playlist"))
            .json(&PlayPlaylistRequest {
                playlist_id: "p1".to_string(),
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(started["playback"]["state"], "playing");
        assert_eq!(started["playback"]["current"]["video_id"], "v1");
        assert_eq!(started["queue_length"], 2);

        let volume: serde_json::Value = client
            .post(format!("{base}/api/volume"))
            .json(&VolumeRequest { volume: 80 })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(volume["playback"]["volume"], 80);

        let clamped: serde_json::Value = client
            .post(format!("{base}/api/volume"))
            .json(&VolumeRequest { volume: 255 })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(clamped["playback"]["volume"], 100);

        let paused: serde_json::Value = client
            .post(format!("{base}/api/pause"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(paused["playback"]["state"], "paused");

        let next: serde_json::Value = client
            .post(format!("{base}/api/next"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(next["playback"]["current"]["video_id"], "v2");

        let previous: serde_json::Value = client
            .post(format!("{base}/api/previous"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(previous["playback"]["current"]["video_id"], "v1");

        let queue: serde_json::Value = client
            .get(format!("{base}/api/queue"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(queue["items"].as_array().unwrap().len(), 2);
        server.abort();
    }

    #[tokio::test]
    async fn queue_mutations() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();
        client
            .post(format!("{base}/api/play/playlist"))
            .json(&PlayPlaylistRequest {
                playlist_id: "p1".to_string(),
            })
            .send()
            .await
            .unwrap();
        let added: serde_json::Value = client
            .post(format!("{base}/api/queue"))
            .json(&QueueRequest { track: track("v3") })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(added["items"].as_array().unwrap().len(), 3);

        let played_next: serde_json::Value = client
            .post(format!("{base}/api/queue/play-next"))
            .json(&PlayNextRequest { index: 2 })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(played_next["items"][1]["track"]["video_id"], "v3");

        let reordered: serde_json::Value = client
            .post(format!("{base}/api/queue/reorder"))
            .json(&ReorderQueueRequest { from: 0, to: 1 })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(reordered["items"][0]["track"]["video_id"], "v3");

        let removed: serde_json::Value = client
            .post(format!("{base}/api/queue/remove"))
            .json(&RemoveQueueRequest { index: 0 })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(removed["items"].as_array().unwrap().len(), 2);

        let cleared: serde_json::Value = client
            .delete(format!("{base}/api/queue"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(cleared["items"].as_array().unwrap().len(), 0);
        server.abort();
    }

    #[tokio::test]
    async fn play_track_transport_and_modes() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();

        let started: serde_json::Value = client
            .post(format!("{base}/api/play/track"))
            .json(&PlayTrackRequest { track: track("v9") })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(started["playback"]["state"], "playing");
        assert_eq!(started["playback"]["current"]["video_id"], "v9");

        let seeked: serde_json::Value = client
            .post(format!("{base}/api/seek"))
            .json(&SeekRequest {
                position_secs: 42.5,
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(seeked["playback"]["position_secs"], 42.5);

        let shuffled: serde_json::Value = client
            .post(format!("{base}/api/shuffle"))
            .json(&ShuffleRequest { enabled: true })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(shuffled["playback"]["shuffle"], true);

        let repeated: serde_json::Value = client
            .post(format!("{base}/api/repeat"))
            .json(&RepeatRequest {
                repeat: ApiRepeatMode::All,
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(repeated["playback"]["repeat"], "all");

        let paused: serde_json::Value = client
            .post(format!("{base}/api/pause"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(paused["playback"]["state"], "paused");

        let resumed: serde_json::Value = client
            .post(format!("{base}/api/resume"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(resumed["playback"]["state"], "playing");
        server.abort();
    }

    #[tokio::test]
    async fn invalid_requests_are_rejected() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();

        let empty_search = client
            .get(format!("{base}/api/search?q="))
            .send()
            .await
            .unwrap();
        assert_eq!(empty_search.status(), reqwest::StatusCode::BAD_REQUEST);

        let bad_seek = client
            .post(format!("{base}/api/seek"))
            .json(&SeekRequest {
                position_secs: -1.0,
            })
            .send()
            .await
            .unwrap();
        assert_eq!(bad_seek.status(), reqwest::StatusCode::BAD_REQUEST);

        let bad_remove = client
            .post(format!("{base}/api/queue/remove"))
            .json(&RemoveQueueRequest { index: 99 })
            .send()
            .await
            .unwrap();
        assert_eq!(bad_remove.status(), reqwest::StatusCode::BAD_REQUEST);

        let bad_reorder = client
            .post(format!("{base}/api/queue/reorder"))
            .json(&ReorderQueueRequest { from: 99, to: 0 })
            .send()
            .await
            .unwrap();
        assert_eq!(bad_reorder.status(), reqwest::StatusCode::BAD_REQUEST);

        // Pause/resume are idempotent even when nothing is playing.
        let idle_pause = client
            .post(format!("{base}/api/pause"))
            .send()
            .await
            .unwrap();
        assert_eq!(idle_pause.status(), reqwest::StatusCode::OK);
        let idle_status: serde_json::Value = idle_pause.json().await.unwrap();
        assert_eq!(idle_status["playback"]["state"], "stopped");
        server.abort();
    }

    #[tokio::test]
    async fn upstream_failures_are_502_with_envelope() {
        let (base, server) = start_server(test_state_with(Arc::new(FailingMusicApi))).await;
        let response = reqwest::Client::new()
            .get(format!("{base}/api/playlists"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_GATEWAY);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["error"]["code"], "upstream_error");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("upstream unavailable")
        );
        server.abort();
    }

    #[tokio::test]
    async fn sse_sends_snapshot_then_live_updates() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{base}/api/events"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/event-stream")
        );
        assert_eq!(response.headers()["cache-control"], "no-cache");
        assert_eq!(response.headers()["x-accel-buffering"], "no");

        let mut stream = Box::pin(response.bytes_stream());
        let mut buffer = String::new();
        read_until(&mut stream, &mut buffer, "event: snapshot").await;
        assert!(buffer.contains("\"type\":\"snapshot\""));
        assert!(buffer.contains("\"state\":\"stopped\""));

        client
            .post(format!("{base}/api/volume"))
            .json(&VolumeRequest { volume: 80 })
            .send()
            .await
            .unwrap();
        read_until(&mut stream, &mut buffer, "\"volume\":80").await;
        assert!(buffer.contains("\"type\":\"playback\""));
        server.abort();
    }

    #[tokio::test]
    async fn sse_replays_events_after_last_event_id() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();

        let response = client
            .get(format!("{base}/api/events"))
            .send()
            .await
            .unwrap();
        let mut stream = Box::pin(response.bytes_stream());
        let mut buffer = String::new();
        read_until(&mut stream, &mut buffer, "event: snapshot").await;

        client
            .post(format!("{base}/api/volume"))
            .json(&VolumeRequest { volume: 80 })
            .send()
            .await
            .unwrap();
        read_until(&mut stream, &mut buffer, "\"volume\":80").await;
        let resume_from = last_event_id(&buffer);
        drop(stream);

        // This update happens while the client is disconnected.
        client
            .post(format!("{base}/api/volume"))
            .json(&VolumeRequest { volume: 60 })
            .send()
            .await
            .unwrap();

        let response = client
            .get(format!("{base}/api/events"))
            .header("Last-Event-ID", resume_from.to_string())
            .send()
            .await
            .unwrap();
        let mut stream = Box::pin(response.bytes_stream());
        let mut replay = String::new();
        read_until(&mut stream, &mut replay, "\"volume\":60").await;
        assert!(!replay.contains("event: snapshot"));
        assert!(last_event_id(&replay) > resume_from);
        server.abort();
    }

    #[tokio::test]
    async fn sse_unknown_last_event_id_resyncs_with_snapshot() {
        let (base, server) = start_server(test_state()).await;
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{base}/api/events"))
            .header("Last-Event-ID", "9999")
            .send()
            .await
            .unwrap();
        let mut stream = Box::pin(response.bytes_stream());
        let mut buffer = String::new();
        read_until(&mut stream, &mut buffer, "event: snapshot").await;
        assert_eq!(last_event_id(&buffer), 0);
        server.abort();
    }

    #[tokio::test]
    async fn missing_playlist_is_404() {
        let (base, server) = start_server(test_state()).await;
        let response = reqwest::Client::new()
            .get(format!("{base}/api/playlists/missing"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        server.abort();
    }
}
