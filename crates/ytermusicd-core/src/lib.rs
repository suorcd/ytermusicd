//! Headless core for `ytermusicd`: paths, configuration and authentication.

pub mod auth;
pub mod config;
pub mod paths;

pub use auth::{Auth, AuthError, AuthState, AuthStatus, LoadedAuth, resolve_auth};
pub use config::{ConfigError, DaemonConfig, Downloader};
pub use paths::{Paths, PathsError};
