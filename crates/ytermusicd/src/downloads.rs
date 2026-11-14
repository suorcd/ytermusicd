use database::YTLocalDatabase;
use download_manager::{
    DownloadManager, DownloadManagerMessage, DownloadSummary as ManagerSummary, Downloader,
    MessageHandler,
};
use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::sync::Mutex as AsyncMutex;
use ytermusicd_core::{AudioSink, PlaybackService, Track};
use ytpapi2::YoutubeMusicVideoRef;

/// Download operations needed by the HTTP layer. A trait keeps API tests
/// independent of `yt-dlp`, the filesystem and the real download manager.
pub trait DownloadControl: Send + Sync {
    fn queue_playlist(&self, tracks: &[Track]);
    fn queue_tracks(&self, tracks: &[Track]);
    fn retry(&self, video_id: &str) -> bool;
    fn summary(&self) -> ManagerSummary;
}

struct Inner {
    manager: &'static DownloadManager,
    tracks: Mutex<HashMap<String, YoutubeMusicVideoRef>>,
}

/// Owns the daemon's download manager. It remembers track metadata so a failed
/// download can be retried without asking the UI to resend the whole track.
#[derive(Clone)]
pub struct DownloadService {
    inner: Arc<Inner>,
}

impl DownloadService {
    pub fn new(cache_dir: PathBuf, parallel_downloads: u16, downloader: Downloader) -> Self {
        let database: &'static YTLocalDatabase =
            Box::leak(Box::new(YTLocalDatabase::new(cache_dir.clone())));
        let manager: &'static DownloadManager = Box::leak(Box::new(DownloadManager::new(
            cache_dir,
            database,
            parallel_downloads,
            downloader,
        )));
        Self {
            inner: Arc::new(Inner {
                manager,
                tracks: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Spawn the download workers and forward status updates into playback.
    /// The future must be created inside a Tokio runtime.
    pub fn spawn<S>(&self, playback: Arc<AsyncMutex<PlaybackService<S>>>)
    where
        S: AudioSink + Send + 'static,
    {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let handler: MessageHandler = Arc::new(move |message| match message {
            DownloadManagerMessage::VideoStatusUpdate(video_id, status) => {
                let _ = sender.send((video_id, status));
            }
        });
        self.inner.manager.spawn_system(Never, handler);

        tokio::spawn(async move {
            while let Some((video_id, status)) = receiver.recv().await {
                let mut playback = playback.lock().await;
                if status == common_structs::MusicDownloadStatus::DownloadFailed {
                    playback.on_download_failed(&video_id);
                } else {
                    playback.set_download_status(&video_id, status);
                }
            }
        });
    }

    fn remember(&self, tracks: &[Track]) -> Vec<YoutubeMusicVideoRef> {
        let mut known = self.inner.tracks.lock().unwrap();
        tracks
            .iter()
            .map(|track| {
                let reference = video_ref(track);
                known.insert(reference.video_id.clone(), reference.clone());
                reference
            })
            .collect()
    }
}

impl DownloadControl for DownloadService {
    fn queue_playlist(&self, tracks: &[Track]) {
        let references = self.remember(tracks);
        self.inner.manager.set_download_list(references);
    }

    fn queue_tracks(&self, tracks: &[Track]) {
        let references = self.remember(tracks);
        self.inner.manager.add_to_download_list(references);
    }

    fn retry(&self, video_id: &str) -> bool {
        let track = self.inner.tracks.lock().unwrap().get(video_id).cloned();
        match track {
            Some(track) => {
                self.inner.manager.add_to_download_list([track]);
                true
            }
            None => false,
        }
    }

    fn summary(&self) -> ManagerSummary {
        self.inner.manager.summary()
    }
}

pub fn video_ref(track: &Track) -> YoutubeMusicVideoRef {
    YoutubeMusicVideoRef {
        video_id: track.video_id.clone(),
        title: track.title.clone(),
        author: track.author.clone(),
        album: track.album.clone(),
        duration: track.duration.clone(),
    }
}

/// The download workers live as long as the daemon process; this future never
/// completes and is cloned for each worker by `spawn_system`.
#[derive(Clone, Copy)]
struct Never;

impl Future for Never {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_track_to_download_reference() {
        let track = Track {
            video_id: "v1".to_string(),
            title: "Song".to_string(),
            author: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        };
        let reference = video_ref(&track);
        assert_eq!(reference.video_id, "v1");
        assert_eq!(reference.title, "Song");
        assert_eq!(reference.author, "Artist");
        assert_eq!(reference.album, "Album");
        assert_eq!(reference.duration, "3:00");
    }
}
