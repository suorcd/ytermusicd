//! Shared DTOs for the ytermusicd HTTP/SSE API.
//!
//! These types are the API contract. The web UI and MCP proxy should consume
//! the generated TypeScript bindings rather than duplicating shapes.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlaylistDetail {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub struct PlaybackSnapshot {
    pub state: PlaybackState,
    pub current: Option<Track>,
    pub position_secs: f64,
    pub volume: u8,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export)]
pub enum DownloadStatus {
    NotDownloaded,
    Downloading { progress: u8 },
    Downloaded,
    Retrying { attempt: u32 },
    Failed { message: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QueueItem {
    pub track: Track,
    pub download: DownloadStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub struct Queue {
    pub items: Vec<QueueItem>,
    pub current: Option<usize>,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AuthState {
    #[default]
    Ok,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub struct AuthStatus {
    pub state: AuthState,
    pub source: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub struct DownloadSummary {
    pub queued: usize,
    pub active: usize,
    pub completed: usize,
    pub retrying: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Status {
    pub name: String,
    pub version: String,
    pub status: String,
    #[ts(type = "number")]
    pub uptime_seconds: u64,
    pub auth: AuthStatus,
    pub playback: PlaybackSnapshot,
    pub queue_length: usize,
    pub downloads: DownloadSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ErrorEnvelope {
    pub error: ApiError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlayPlaylistRequest {
    pub playlist_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlayTrackRequest {
    pub track: Track,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QueueRequest {
    pub track: Track,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemoveQueueRequest {
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlayNextRequest {
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShuffleRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RepeatRequest {
    pub repeat: RepeatMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VolumeRequest {
    pub volume: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SeekRequest {
    pub position_secs: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReorderQueueRequest {
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum EventKind {
    Snapshot {
        status: Box<Status>,
        queue: Queue,
    },
    Playback {
        playback: PlaybackSnapshot,
    },
    Queue {
        queue: Queue,
    },
    Download {
        video_id: String,
        status: DownloadStatus,
    },
    TrackEnded {
        video_id: String,
    },
    Error {
        error: ApiError,
    },
    LibraryChanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Event {
    #[ts(type = "number")]
    pub id: u64,
    pub event: EventKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> Track {
        Track {
            video_id: "v1".to_string(),
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        }
    }

    #[test]
    fn track_round_trip() {
        let track = track();
        let json = serde_json::to_string(&track).unwrap();
        assert_eq!(serde_json::from_str::<Track>(&json).unwrap(), track);
    }

    #[test]
    fn download_status_is_tagged() {
        let status = DownloadStatus::Downloading { progress: 42 };
        let json = serde_json::to_value(status).unwrap();
        assert_eq!(json["state"], "downloading");
        assert_eq!(json["progress"], 42);
    }

    #[test]
    fn event_is_tagged() {
        let event = Event {
            id: 7,
            event: EventKind::Download {
                video_id: "v1".to_string(),
                status: DownloadStatus::Downloaded,
            },
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["id"], 7);
        assert_eq!(json["event"]["type"], "download");
        assert_eq!(json["event"]["status"]["state"], "downloaded");
    }

    #[test]
    fn status_round_trip() {
        let status = Status {
            name: "ytermusicd".to_string(),
            version: "0.1.0".to_string(),
            status: "ok".to_string(),
            uptime_seconds: 12,
            auth: AuthStatus::default(),
            playback: PlaybackSnapshot::default(),
            queue_length: 0,
            downloads: DownloadSummary::default(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(serde_json::from_str::<Status>(&json).unwrap(), status);
    }
}
