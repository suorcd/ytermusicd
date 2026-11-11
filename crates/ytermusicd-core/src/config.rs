use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Downloader {
    #[default]
    Ytdlp,
    RustyYtdl,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct DaemonConfig {
    /// Bind address for the HTTP API. CLI/env override this.
    pub bind: Option<String>,
    /// Port for the HTTP API. CLI/env override this.
    pub port: Option<u16>,
    /// Explicit headers file. CLI/env override this.
    pub headers: Option<PathBuf>,
    /// Maximum number of parallel downloads.
    pub parallel_downloads: u16,
    pub downloader: Downloader,
    /// Enable MPRIS/D-Bus media controls.
    pub dbus: bool,
    /// Initial volume, 0-100.
    pub initial_volume: u8,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind: None,
            port: None,
            headers: None,
            parallel_downloads: 4,
            downloader: Downloader::Ytdlp,
            dbus: true,
            initial_volume: 50,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid config {}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
}

impl DaemonConfig {
    /// Load and validate a config file. Parse and validation errors are fatal.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let config: Self = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        config.validate(path)?;
        Ok(config)
    }

    /// Load a config file, using defaults only when the file does not exist.
    pub fn load_or_default(path: &Path) -> Result<Self, ConfigError> {
        match fs::metadata(path) {
            Ok(_) => Self::load(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn validate(&self, path: &Path) -> Result<(), ConfigError> {
        if self.parallel_downloads == 0 {
            return Err(ConfigError::Invalid {
                path: path.to_path_buf(),
                message: "parallel_downloads must be at least 1".to_string(),
            });
        }
        if self.initial_volume > 100 {
            return Err(ConfigError::Invalid {
                path: path.to_path_buf(),
                message: "initial_volume must be between 0 and 100".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_sane() {
        let config = DaemonConfig::default();
        assert_eq!(config.parallel_downloads, 4);
        assert_eq!(config.downloader, Downloader::Ytdlp);
        assert!(config.dbus);
        assert_eq!(config.initial_volume, 50);
        assert_eq!(config.bind, None);
        assert_eq!(config.port, None);
        assert_eq!(config.headers, None);
    }

    #[test]
    fn loads_valid_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
bind = "0.0.0.0"
port = 9000
headers = "/tmp/headers.txt"
parallel_downloads = 2
downloader = "rusty_ytdl"
dbus = false
initial_volume = 75
"#,
        )
        .unwrap();

        let config = DaemonConfig::load(&path).unwrap();
        assert_eq!(config.bind.as_deref(), Some("0.0.0.0"));
        assert_eq!(config.port, Some(9000));
        assert_eq!(config.headers, Some(PathBuf::from("/tmp/headers.txt")));
        assert_eq!(config.parallel_downloads, 2);
        assert_eq!(config.downloader, Downloader::RustyYtdl);
        assert!(!config.dbus);
        assert_eq!(config.initial_volume, 75);
    }

    #[test]
    fn missing_file_uses_defaults() {
        let dir = tempdir().unwrap();
        let config = DaemonConfig::load_or_default(&dir.path().join("missing.toml")).unwrap();
        assert_eq!(config, DaemonConfig::default());
    }

    #[test]
    fn parse_errors_include_the_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "port = [").unwrap();

        let error = DaemonConfig::load(&path).unwrap_err().to_string();
        assert!(error.contains("config.toml"), "{error}");
        assert!(error.contains("failed to parse"), "{error}");
    }

    #[test]
    fn rejects_invalid_parallel_downloads() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "parallel_downloads = 0").unwrap();

        let error = DaemonConfig::load(&path).unwrap_err().to_string();
        assert!(error.contains("parallel_downloads"), "{error}");
    }

    #[test]
    fn rejects_invalid_volume() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "initial_volume = 101").unwrap();

        let error = DaemonConfig::load(&path).unwrap_err().to_string();
        assert!(error.contains("initial_volume"), "{error}");
    }
}
