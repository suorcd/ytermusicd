mod task;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use database::YTLocalDatabase;
use tokio::{select, sync::Notify, task::JoinHandle};
use ytpapi2::YoutubeMusicVideoRef;

use common_structs::MusicDownloadStatus;

pub use task::DownloadError;

pub type MessageHandler = Arc<dyn Fn(DownloadManagerMessage) + Send + Sync + 'static>;

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadManagerMessage {
    VideoStatusUpdate(String, MusicDownloadStatus),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Downloader {
    YtDlp,
    #[cfg(feature = "rusty-ytdl-backend")]
    RustyYtdl,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DownloadSummary {
    pub queued: usize,
    pub active: usize,
    pub completed: usize,
    pub retrying: usize,
    pub failed: usize,
}

pub struct DownloadManager {
    database: &'static YTLocalDatabase,
    cache_dir: PathBuf,
    parallel_downloads: u16,
    pub downloader: Downloader,
    yt_dlp_path: PathBuf,
    max_retries: u32,
    retry_delay: Duration,
    handles: Mutex<Vec<JoinHandle<()>>>,
    download_list: Mutex<VecDeque<YoutubeMusicVideoRef>>,
    in_download: Mutex<HashSet<String>>,
    statuses: Mutex<HashMap<String, MusicDownloadStatus>>,
    notify: Notify,
}

impl DownloadManager {
    pub fn new(
        cache_dir: PathBuf,
        database: &'static YTLocalDatabase,
        parallel_downloads: u16,
        downloader: Downloader,
    ) -> Self {
        Self {
            database,
            cache_dir,
            parallel_downloads,
            downloader,
            yt_dlp_path: PathBuf::from("yt-dlp"),
            max_retries: 2,
            retry_delay: Duration::from_millis(500),
            handles: Mutex::new(Vec::new()),
            download_list: Mutex::new(VecDeque::new()),
            in_download: Mutex::new(HashSet::new()),
            statuses: Mutex::new(HashMap::new()),
            notify: Notify::new(),
        }
    }

    pub fn with_yt_dlp_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.yt_dlp_path = path.into();
        self
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    pub fn status(&self, video_id: &str) -> Option<MusicDownloadStatus> {
        self.statuses.lock().unwrap().get(video_id).copied()
    }

    pub fn active_downloads(&self) -> usize {
        self.in_download.lock().unwrap().len()
    }

    pub fn summary(&self) -> DownloadSummary {
        let mut summary = DownloadSummary {
            queued: self.download_list.lock().unwrap().len(),
            active: self.in_download.lock().unwrap().len(),
            ..DownloadSummary::default()
        };
        for status in self.statuses.lock().unwrap().values() {
            match status {
                MusicDownloadStatus::Downloaded => summary.completed += 1,
                MusicDownloadStatus::Retrying(_) => summary.retrying += 1,
                MusicDownloadStatus::DownloadFailed => summary.failed += 1,
                _ => {}
            }
        }
        summary
    }

    pub fn remove_from_in_downloads(&self, video: &String) {
        self.in_download.lock().unwrap().remove(video);
    }

    fn take(&self) -> Option<YoutubeMusicVideoRef> {
        self.download_list.lock().unwrap().pop_front()
    }

    /// This has to be called as a service stream
    /// HANDLES.lock().unwrap().push(run_service(async move {
    ///     run_service_stream(sender);
    /// }));
    pub fn run_service_stream(
        &'static self,
        cancelation: impl Future<Output = ()> + Clone + Send + 'static,
        sender: MessageHandler,
    ) {
        let fut = async move {
            loop {
                if let Some(id) = self.take() {
                    self.start_download(id, sender.clone()).await;
                } else {
                    self.notify.notified().await;
                }
            }
        };
        let service = tokio::task::spawn(async move {
            select! {
                _ = fut => {},
                _ = cancelation => {},
            }
        });
        self.handles.lock().unwrap().push(service);
    }

    pub fn spawn_system(
        &'static self,
        cancelation: impl Future<Output = ()> + Clone + Send + 'static,
        sender: MessageHandler,
    ) {
        for _ in 0..self.parallel_downloads {
            self.run_service_stream(cancelation.clone(), sender.clone());
        }
    }

    pub fn clean(
        &'static self,
        cancelation: impl Future<Output = ()> + Clone + Send + 'static,
        sender: MessageHandler,
    ) {
        self.download_list.lock().unwrap().clear();
        self.in_download.lock().unwrap().clear();
        {
            let mut handle = self.handles.lock().unwrap();
            for i in handle.iter() {
                i.abort()
            }
            handle.clear();
        }
        self.spawn_system(cancelation, sender);
    }

    pub fn set_download_list(&self, to_add: impl IntoIterator<Item = YoutubeMusicVideoRef>) {
        let mut list = self.download_list.lock().unwrap();
        list.clear();
        list.extend(to_add);
        drop(list);
        self.notify.notify_one();
    }

    pub fn add_to_download_list(&self, to_add: impl IntoIterator<Item = YoutubeMusicVideoRef>) {
        let mut list = self.download_list.lock().unwrap();
        let was_empty = list.is_empty();
        list.extend(to_add);
        drop(list);
        if was_empty {
            self.notify.notify_one();
        }
    }

    fn set_status(&self, video_id: &str, status: MusicDownloadStatus) {
        self.statuses
            .lock()
            .unwrap()
            .insert(video_id.to_string(), status);
    }

    pub async fn start_download(&self, song: YoutubeMusicVideoRef, s: MessageHandler) -> bool {
        {
            let mut downloads = self.in_download.lock().unwrap();
            if downloads.contains(&song.video_id) {
                return false;
            }
            downloads.insert(song.video_id.clone());
        }

        let _guard = InDownloadGuard {
            manager: self,
            video_id: song.video_id.clone(),
        };
        self.start_download_inner(song, s).await
    }

    async fn start_download_inner(&self, song: YoutubeMusicVideoRef, s: MessageHandler) -> bool {
        let id = song.video_id.clone();
        self.set_status(&id, MusicDownloadStatus::Downloading(1));
        s(DownloadManagerMessage::VideoStatusUpdate(
            id.clone(),
            MusicDownloadStatus::Downloading(1),
        ));

        let downloads = self.cache_dir.join("downloads");
        let mp4 = downloads.join(format!("{id}.mp4"));
        let json = downloads.join(format!("{id}.json"));
        let part = downloads.join(format!("{id}.part"));

        if json.exists() {
            self.set_status(&id, MusicDownloadStatus::Downloaded);
            s(DownloadManagerMessage::VideoStatusUpdate(
                id,
                MusicDownloadStatus::Downloaded,
            ));
            return true;
        }

        let _ = std::fs::remove_file(&mp4);
        let _ = std::fs::remove_file(&part);

        let mut attempt = 0;
        loop {
            let reporter = {
                let manager = self;
                let id = id.clone();
                let s = s.clone();
                move |progress: usize| {
                    let status = MusicDownloadStatus::Downloading(progress.min(100));
                    manager.set_status(&id, status);
                    s(DownloadManagerMessage::VideoStatusUpdate(
                        id.clone(),
                        status,
                    ));
                }
            };

            match self.handle_download(&id, &part, &reporter).await {
                Ok(()) => {
                    if let Err(error) = self.finalize(&song, &part, &mp4, &json) {
                        log::error!("couldn't finalize download for {}: {error}", song.video_id);
                        let _ = std::fs::remove_file(&mp4);
                        let _ = std::fs::remove_file(&part);
                        self.set_status(&id, MusicDownloadStatus::DownloadFailed);
                        s(DownloadManagerMessage::VideoStatusUpdate(
                            id,
                            MusicDownloadStatus::DownloadFailed,
                        ));
                        return false;
                    }

                    self.set_status(&id, MusicDownloadStatus::Downloaded);
                    s(DownloadManagerMessage::VideoStatusUpdate(
                        id,
                        MusicDownloadStatus::Downloaded,
                    ));
                    return true;
                }
                Err(error) => {
                    let _ = std::fs::remove_file(&part);
                    if attempt < self.max_retries {
                        attempt += 1;
                        self.set_status(&id, MusicDownloadStatus::Retrying(attempt));
                        s(DownloadManagerMessage::VideoStatusUpdate(
                            id.clone(),
                            MusicDownloadStatus::Retrying(attempt),
                        ));
                        tokio::time::sleep(self.retry_delay).await;
                    } else {
                        self.set_status(&id, MusicDownloadStatus::DownloadFailed);
                        s(DownloadManagerMessage::VideoStatusUpdate(
                            id.clone(),
                            MusicDownloadStatus::DownloadFailed,
                        ));
                        log::error!("couldn't download {id}: {error}");
                        return false;
                    }
                }
            }
        }
    }

    fn finalize(
        &self,
        song: &YoutubeMusicVideoRef,
        part: &std::path::Path,
        mp4: &std::path::Path,
        json: &std::path::Path,
    ) -> Result<(), DownloadError> {
        std::fs::rename(part, mp4)?;
        let serialized = serde_json::to_string(song)
            .map_err(|error| DownloadError::IoError(std::io::Error::other(error)))?;
        std::fs::write(json, serialized)?;
        self.database.append(song.clone());
        Ok(())
    }

    pub fn start_task_unary(
        &'static self,
        s: MessageHandler,
        song: YoutubeMusicVideoRef,
        cancelation: impl Future<Output = ()> + Send + 'static,
    ) {
        let fut = async move {
            self.start_download(song, s).await;
        };
        let service = tokio::task::spawn(async move {
            tokio::select! {
                _ = fut => {},
                _ = cancelation => {},
            }
        });
        self.handles.lock().unwrap().push(service);
    }
}

struct InDownloadGuard<'a> {
    manager: &'a DownloadManager,
    video_id: String,
}

impl Drop for InDownloadGuard<'_> {
    fn drop(&mut self) {
        self.manager
            .in_download
            .lock()
            .unwrap()
            .remove(&self.video_id);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use tempfile::{TempDir, tempdir};
    use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};

    static TEST_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

    async fn serial() -> AsyncMutexGuard<'static, ()> {
        TEST_LOCK.lock().await
    }

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("fake-yt-dlp");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn song(id: &str) -> YoutubeMusicVideoRef {
        YoutubeMusicVideoRef {
            video_id: id.to_string(),
            title: format!("Track {id}"),
            author: "Artist".to_string(),
            album: "Album".to_string(),
            duration: "3:00".to_string(),
        }
    }

    fn manager(dir: &TempDir, script: PathBuf, max_retries: u32) -> (DownloadManager, PathBuf) {
        let cache = dir.path().join("cache");
        let downloads = cache.join("downloads");
        fs::create_dir_all(&downloads).unwrap();
        let database: &'static YTLocalDatabase =
            Box::leak(Box::new(YTLocalDatabase::new(cache.clone())));
        let manager = DownloadManager::new(cache, database, 1, Downloader::YtDlp)
            .with_yt_dlp_path(script)
            .with_max_retries(max_retries)
            .with_retry_delay(Duration::ZERO);
        (manager, downloads)
    }

    fn collector() -> (MessageHandler, Arc<Mutex<Vec<DownloadManagerMessage>>>) {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let handler: MessageHandler = Arc::new(move |message| {
            sink.lock().unwrap().push(message);
        });
        (handler, messages)
    }

    fn success_script(progress: bool) -> String {
        let progress = if progress {
            "echo 'download:  0.0%'\necho 'download: 50.0%'"
        } else {
            ""
        };
        format!(
            "out=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in -o) out=\"$2\"; shift 2;; *) shift;; esac; done\n{progress}\necho fake-audio > \"$out\"\necho 'download:100.0%'"
        )
    }

    #[tokio::test]
    async fn successful_download_is_atomic_and_reports_progress() {
        let _guard = serial().await;
        let dir = tempdir().unwrap();
        let path = script(dir.path(), &success_script(true));
        let (manager, downloads) = manager(&dir, path, 0);
        let (handler, messages) = collector();

        assert!(manager.start_download(song("v1"), handler).await);
        assert!(downloads.join("v1.mp4").exists());
        assert!(downloads.join("v1.json").exists());
        assert!(!downloads.join("v1.part").exists());
        assert_eq!(manager.status("v1"), Some(MusicDownloadStatus::Downloaded));
        assert_eq!(manager.active_downloads(), 0);
        assert_eq!(manager.summary().completed, 1);

        let messages = messages.lock().unwrap();
        assert!(messages.iter().any(|message| matches!(
            message,
            DownloadManagerMessage::VideoStatusUpdate(_, MusicDownloadStatus::Downloading(50))
        )));
        assert!(messages.iter().any(|message| matches!(
            message,
            DownloadManagerMessage::VideoStatusUpdate(_, MusicDownloadStatus::Downloaded)
        )));
    }

    #[tokio::test]
    async fn failed_download_cleans_up_and_records_failure() {
        let _guard = serial().await;
        let dir = tempdir().unwrap();
        let path = script(dir.path(), "echo boom >&2\nexit 1");
        let (manager, downloads) = manager(&dir, path, 0);
        let (handler, _messages) = collector();

        assert!(!manager.start_download(song("v1"), handler).await);
        assert!(!downloads.join("v1.mp4").exists());
        assert!(!downloads.join("v1.json").exists());
        assert!(!downloads.join("v1.part").exists());
        assert_eq!(
            manager.status("v1"),
            Some(MusicDownloadStatus::DownloadFailed)
        );
        assert_eq!(manager.active_downloads(), 0);
        assert_eq!(manager.summary().failed, 1);
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let _guard = serial().await;
        let dir = tempdir().unwrap();
        let lock = dir.path().join("first-attempt-lock");
        let body = format!(
            "out=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in -o) out=\"$2\"; shift 2;; *) shift;; esac; done\nif mkdir {lock} 2>/dev/null; then echo 'temporary failure' >&2; exit 1; fi\necho fake-audio > \"$out\"\necho 'download:100.0%'",
            lock = lock.display()
        );
        let path = script(dir.path(), &body);
        let (manager, downloads) = manager(&dir, path, 1);
        let (handler, messages) = collector();

        assert!(manager.start_download(song("v1"), handler).await);
        assert!(downloads.join("v1.mp4").exists());
        assert_eq!(manager.status("v1"), Some(MusicDownloadStatus::Downloaded));
        assert_eq!(manager.active_downloads(), 0);
        let messages = messages.lock().unwrap();
        assert!(messages.iter().any(|message| matches!(
            message,
            DownloadManagerMessage::VideoStatusUpdate(_, MusicDownloadStatus::Retrying(1))
        )));
    }

    #[tokio::test]
    async fn already_downloaded_is_a_noop() {
        let _guard = serial().await;
        let dir = tempdir().unwrap();
        let marker = dir.path().join("invoked");
        let body = format!("touch {}", marker.display());
        let path = script(dir.path(), &body);
        let (manager, downloads) = manager(&dir, path, 0);
        fs::write(downloads.join("v1.json"), "{}").unwrap();
        let (handler, _messages) = collector();

        assert!(manager.start_download(song("v1"), handler).await);
        assert!(!marker.exists());
        assert_eq!(manager.status("v1"), Some(MusicDownloadStatus::Downloaded));
        assert_eq!(manager.active_downloads(), 0);
    }

    #[tokio::test]
    async fn cancellation_clears_active_download_state() {
        let _guard = serial().await;
        let dir = tempdir().unwrap();
        let path = script(dir.path(), "sleep 2");
        let (manager, _downloads) = manager(&dir, path, 0);
        let (handler, _messages) = collector();

        let mut future = Box::pin(manager.start_download(song("v1"), handler));
        tokio::select! {
            _ = &mut future => panic!("download unexpectedly completed"),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
        drop(future);

        assert_eq!(manager.active_downloads(), 0);
    }
}
