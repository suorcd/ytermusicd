use std::path::Path;
use std::process::Stdio;

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
};

use crate::{DownloadManager, Downloader};

#[derive(Debug)]
pub enum DownloadError {
    YtDlpFailed(String),
    IoError(std::io::Error),
    #[cfg(feature = "rusty-ytdl-backend")]
    RustyYtdl(rusty_ytdl::VideoError),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::YtDlpFailed(msg) => write!(f, "yt-dlp failed: {}", msg),
            DownloadError::IoError(e) => write!(f, "IO error: {}", e),
            #[cfg(feature = "rusty-ytdl-backend")]
            DownloadError::RustyYtdl(e) => write!(f, "rusty_ytdl error: {}", e),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::IoError(e)
    }
}

#[cfg(feature = "rusty-ytdl-backend")]
impl From<rusty_ytdl::VideoError> for DownloadError {
    fn from(e: rusty_ytdl::VideoError) -> Self {
        DownloadError::RustyYtdl(e)
    }
}

pub(crate) fn parse_ytdlp_progress(line: &str) -> Option<usize> {
    let value = line
        .trim()
        .strip_prefix("download:")?
        .trim()
        .trim_end_matches('%');
    value
        .parse::<f64>()
        .ok()
        .map(|percent| percent.clamp(0.0, 100.0) as usize)
}

async fn download_with_ytdlp(
    yt_dlp: &Path,
    video_id: &str,
    output_path: &Path,
    progress: &(dyn Fn(usize) + Send + Sync),
) -> Result<(), DownloadError> {
    let url = format!("https://www.youtube.com/watch?v={}", video_id);
    let output = output_path.to_string_lossy().into_owned();
    progress(0);

    let mut child = Command::new(yt_dlp)
        .args([
            "--no-playlist",
            "-f",
            "bestaudio[ext=m4a]/bestaudio[ext=mp4]/bestaudio",
            "--merge-output-format",
            "mp4",
            "-o",
            &output,
            "--newline",
            "--progress-template",
            "download:%(progress._percent_str)s",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().ok_or_else(|| {
        DownloadError::IoError(std::io::Error::other("yt-dlp stdout was not captured"))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        DownloadError::IoError(std::io::Error::other("yt-dlp stderr was not captured"))
    })?;

    let stderr_task = tokio::spawn(async move {
        let mut buffer = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut buffer).await;
        buffer
    });

    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        if let Some(percent) = parse_ytdlp_progress(&line) {
            progress(percent);
        }
    }

    let status = child.wait().await?;
    let stderr = stderr_task.await.unwrap_or_default();
    if !status.success() {
        return Err(DownloadError::YtDlpFailed(stderr));
    }

    progress(100);
    Ok(())
}

#[cfg(feature = "rusty-ytdl-backend")]
async fn download_with_rusty_ytdl(
    video_id: &str,
    output_path: &Path,
    progress: &(dyn Fn(usize) + Send + Sync),
) -> Result<(), DownloadError> {
    use rusty_ytdl::{DownloadOptions, Video, VideoOptions, VideoQuality, VideoSearchOptions};
    use std::io::Write;
    use std::sync::Arc;

    let search_options = VideoSearchOptions::Custom(Arc::new(|format| {
        format.has_audio && !format.has_video && format.mime_type.container == "mp4"
    }));
    let video_options = VideoOptions {
        quality: VideoQuality::Custom(
            search_options.clone(),
            Arc::new(|x, y| x.audio_bitrate.cmp(&y.audio_bitrate)),
        ),
        filter: search_options,
        download_options: DownloadOptions {
            dl_chunk_size: Some(1024 * 100_u64),
        },
        ..Default::default()
    };

    let video = Video::new_with_options(video_id, video_options)?;
    progress(0);

    let stream = video.stream().await?;
    let length = stream.content_length();

    let mut file = std::fs::File::create(output_path)
        .map_err(|e| rusty_ytdl::VideoError::DownloadError(e.to_string()))?;

    let mut total = 0;
    while let Some(chunk) = stream.chunk().await? {
        total += chunk.len();
        progress((total as f64 / length as f64 * 100.0) as usize);
        file.write_all(&chunk)
            .map_err(|e| rusty_ytdl::VideoError::DownloadError(e.to_string()))?;
    }

    file.flush()
        .map_err(|e| rusty_ytdl::VideoError::DownloadError(e.to_string()))?;

    if total != length || length == 0 {
        std::fs::remove_file(output_path)
            .map_err(|e| rusty_ytdl::VideoError::DownloadError(e.to_string()))?;
        return Err(rusty_ytdl::VideoError::DownloadError(format!(
            "Downloaded file is not the same size as the content length ({}/{})",
            total, length
        ))
        .into());
    }

    progress(100);
    Ok(())
}

impl DownloadManager {
    pub(crate) async fn handle_download(
        &self,
        id: &str,
        output: &Path,
        progress: &(dyn Fn(usize) + Send + Sync),
    ) -> Result<(), DownloadError> {
        match self.downloader {
            Downloader::YtDlp => download_with_ytdlp(&self.yt_dlp_path, id, output, progress).await,
            #[cfg(feature = "rusty-ytdl-backend")]
            Downloader::RustyYtdl => download_with_rusty_ytdl(id, output, progress).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ytdlp_progress;

    #[test]
    fn parses_progress_lines() {
        assert_eq!(parse_ytdlp_progress("download:  0.0%"), Some(0));
        assert_eq!(parse_ytdlp_progress("download: 42.5%"), Some(42));
        assert_eq!(parse_ytdlp_progress("download:100.0%"), Some(100));
        assert_eq!(parse_ytdlp_progress("download: 150.0%"), Some(100));
        assert_eq!(parse_ytdlp_progress("not progress"), None);
        assert_eq!(parse_ytdlp_progress("download: n/a%"), None);
    }
}
