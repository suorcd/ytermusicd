#[derive(PartialEq, Debug, Clone, Copy)]
pub enum MusicDownloadStatus {
    NotDownloaded,
    Downloaded,
    Downloading(usize),
    Retrying(u32),
    DownloadFailed,
}

impl MusicDownloadStatus {
    pub fn character(&self, playing: Option<bool>) -> String {
        match self {
            Self::NotDownloaded => {
                if let Some(e) = playing {
                    if e { '▶' } else { '⏸' }
                } else {
                    ' '
                }
            }
            Self::Downloaded => ' ',
            Self::Downloading(progress) => return format!("⭳ [{:02}%]", progress),
            Self::Retrying(attempt) => return format!("⟳ [{attempt}]"),
            Self::DownloadFailed => '⚠',
        }
        .into()
    }
}
