//! Headless core for `ytermusicd`: paths, configuration, authentication,
//! playback/queue state and the event bus.

pub mod auth;
pub mod config;
pub mod events;
pub mod model;
pub mod paths;
pub mod playback;

pub use auth::{Auth, AuthError, AuthState, AuthStatus, LoadedAuth, resolve_auth};
pub use config::{ConfigError, DaemonConfig, Downloader};
pub use events::{CoreEvent, EventBus, EventEnvelope, Snapshot};
pub use model::{
    PlaybackSnapshot, PlaybackStatus, Queue, QueueItem, QueueSnapshot, RepeatMode, Track,
};
pub use paths::{Paths, PathsError};
pub use playback::{AudioSink, PlaybackError, PlaybackService};
