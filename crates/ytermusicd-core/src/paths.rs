use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "suorcd";
const APPLICATION: &str = "ytermusicd";
const LEGACY_ORGANIZATION: &str = "ccgauche";
const LEGACY_APPLICATION: &str = "ytermusic";

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("could not determine the user's home directory")]
    NoHome,
}

/// Resolved filesystem locations for the daemon and its legacy client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub legacy_config_dir: PathBuf,
    pub legacy_cache_dir: PathBuf,
    pub legacy_data_dir: PathBuf,
    pub legacy_state_dir: PathBuf,
}

impl Paths {
    /// Discover the platform directories for `ytermusicd` and the legacy TUI.
    pub fn discover() -> Result<Self, PathsError> {
        let current =
            ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).ok_or(PathsError::NoHome)?;
        let legacy = ProjectDirs::from(QUALIFIER, LEGACY_ORGANIZATION, LEGACY_APPLICATION)
            .ok_or(PathsError::NoHome)?;

        Ok(Self {
            config_dir: current.config_dir().to_path_buf(),
            cache_dir: current.cache_dir().to_path_buf(),
            data_dir: current.data_dir().to_path_buf(),
            state_dir: current
                .state_dir()
                .unwrap_or_else(|| current.data_local_dir())
                .to_path_buf(),
            legacy_config_dir: legacy.config_dir().to_path_buf(),
            legacy_cache_dir: legacy.cache_dir().to_path_buf(),
            legacy_data_dir: legacy.data_dir().to_path_buf(),
            legacy_state_dir: legacy
                .state_dir()
                .unwrap_or_else(|| legacy.data_local_dir())
                .to_path_buf(),
        })
    }

    /// Create the directories the daemon writes to.
    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [
            &self.config_dir,
            &self.cache_dir,
            &self.data_dir,
            &self.state_dir,
            &self.downloads_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn headers_file(&self) -> PathBuf {
        self.config_dir.join("headers.txt")
    }

    pub fn legacy_headers_file(&self) -> PathBuf {
        self.legacy_config_dir.join("headers.txt")
    }

    pub fn account_id_file_near(headers: &Path) -> PathBuf {
        headers
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("account_id.txt")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.cache_dir.join("downloads")
    }

    pub fn database_file(&self) -> PathBuf {
        self.cache_dir.join("db.bin")
    }

    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }

    pub fn log_file(&self) -> PathBuf {
        self.state_dir.join("ytermusicd.log")
    }

    /// Project directories used by the legacy TUI, kept for migrations.
    pub fn legacy_project_dirs() -> Option<ProjectDirs> {
        ProjectDirs::from(QUALIFIER, LEGACY_ORGANIZATION, LEGACY_APPLICATION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_paths_are_derived_from_roots() {
        let paths = Paths {
            config_dir: PathBuf::from("/config"),
            cache_dir: PathBuf::from("/cache"),
            data_dir: PathBuf::from("/data"),
            state_dir: PathBuf::from("/state"),
            legacy_config_dir: PathBuf::from("/legacy-config"),
            legacy_cache_dir: PathBuf::from("/legacy-cache"),
            legacy_data_dir: PathBuf::from("/legacy-data"),
            legacy_state_dir: PathBuf::from("/legacy-state"),
        };

        assert_eq!(paths.config_file(), PathBuf::from("/config/config.toml"));
        assert_eq!(paths.headers_file(), PathBuf::from("/config/headers.txt"));
        assert_eq!(
            paths.legacy_headers_file(),
            PathBuf::from("/legacy-config/headers.txt")
        );
        assert_eq!(paths.downloads_dir(), PathBuf::from("/cache/downloads"));
        assert_eq!(paths.database_file(), PathBuf::from("/cache/db.bin"));
        assert_eq!(paths.state_file(), PathBuf::from("/state/state.json"));
        assert_eq!(
            Paths::account_id_file_near(Path::new("/config/headers.txt")),
            PathBuf::from("/config/account_id.txt")
        );
    }
}
