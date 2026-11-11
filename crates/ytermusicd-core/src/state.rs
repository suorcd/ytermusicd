use crate::model::{RepeatMode, Track};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const STATE_VERSION: u32 = 1;

/// Versioned player state persisted between daemon runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub version: u32,
    pub tracks: Vec<Track>,
    pub current: Option<usize>,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub volume: u8,
    pub position_secs: f64,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            tracks: Vec::new(),
            current: None,
            repeat: RepeatMode::Off,
            shuffle: false,
            volume: 50,
            position_secs: 0.0,
        }
    }
}

impl PersistedState {
    /// Write the state atomically via a temporary file in the same directory.
    pub fn save(&self, path: &Path) -> Result<(), StateError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let text = toml::to_string_pretty(self).map_err(StateError::Serialize)?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, text).map_err(|source| StateError::Io {
            path: tmp.clone(),
            source,
        })?;

        if let Err(error) = fs::rename(&tmp, path) {
            // Windows refuses to replace an existing file with rename.
            if path.exists() {
                fs::remove_file(path).map_err(|source| StateError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
                fs::rename(&tmp, path).map_err(|source| StateError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            } else {
                return Err(StateError::Io {
                    path: path.to_path_buf(),
                    source: error,
                });
            }
        }
        Ok(())
    }

    /// Load state. A missing file returns defaults; malformed or incompatible
    /// files return an error so the caller can quarantine them.
    pub fn load(path: &Path) -> Result<Self, StateError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(StateError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        let state: Self = toml::from_str(&text).map_err(|source| StateError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        if state.version != STATE_VERSION {
            return Err(StateError::UnsupportedVersion {
                found: state.version,
                expected: STATE_VERSION,
            });
        }
        state.validate()?;
        Ok(state)
    }

    fn validate(&self) -> Result<(), StateError> {
        if let Some(current) = self.current
            && current >= self.tracks.len()
        {
            return Err(StateError::Invalid(format!(
                "current index {current} is out of range for {} tracks",
                self.tracks.len()
            )));
        }
        if self.volume > 100 {
            return Err(StateError::Invalid(format!(
                "volume {} is outside 0-100",
                self.volume
            )));
        }
        if !self.position_secs.is_finite() || self.position_secs < 0.0 {
            return Err(StateError::Invalid(format!(
                "position {} is invalid",
                self.position_secs
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("failed to access state {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse state {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to serialize state: {0}")]
    Serialize(#[source] toml::ser::Error),
    #[error("unsupported state version {found}, expected {expected}")]
    UnsupportedVersion { found: u32, expected: u32 },
    #[error("invalid state: {0}")]
    Invalid(String),
    #[error("failed to restore state: {0}")]
    Restore(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn track(index: usize) -> Track {
        Track {
            video_id: format!("video-{index}"),
            title: format!("Track {index}"),
            author: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        }
    }

    #[test]
    fn round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = PersistedState {
            tracks: vec![track(0), track(1)],
            current: Some(1),
            repeat: RepeatMode::All,
            shuffle: true,
            volume: 80,
            position_secs: 42.5,
            ..PersistedState::default()
        };

        state.save(&path).unwrap();
        let loaded = PersistedState::load(&path).unwrap();
        assert_eq!(state, loaded);
        assert!(!dir.path().join("state.tmp").exists());
    }

    #[test]
    fn missing_file_uses_defaults() {
        let dir = tempdir().unwrap();
        let state = PersistedState::load(&dir.path().join("missing.json")).unwrap();
        assert_eq!(state, PersistedState::default());
    }

    #[test]
    fn corrupt_file_is_reported() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(&path, "not = [valid").unwrap();
        let error = PersistedState::load(&path).unwrap_err();
        assert!(matches!(error, StateError::Parse { .. }));
        assert!(error.to_string().contains("state.json"));
    }

    #[test]
    fn unsupported_version_is_reported() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = PersistedState {
            version: 99,
            ..PersistedState::default()
        };
        state.save(&path).unwrap();
        let error = PersistedState::load(&path).unwrap_err();
        assert!(matches!(
            error,
            StateError::UnsupportedVersion {
                found: 99,
                expected: 1
            }
        ));
    }

    #[test]
    fn invalid_current_index_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = PersistedState {
            tracks: vec![track(0)],
            current: Some(4),
            ..PersistedState::default()
        };
        state.save(&path).unwrap();
        let error = PersistedState::load(&path).unwrap_err();
        assert!(matches!(error, StateError::Invalid(_)));
    }

    #[test]
    fn invalid_volume_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = PersistedState {
            volume: 101,
            ..PersistedState::default()
        };
        state.save(&path).unwrap();
        let error = PersistedState::load(&path).unwrap_err();
        assert!(error.to_string().contains("volume"));
    }
}
