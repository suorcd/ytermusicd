use crate::{
    events::{CoreEvent, EventBus, Snapshot},
    model::{PlaybackSnapshot, PlaybackStatus, Queue, RepeatMode, Track},
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;

/// Abstraction over the audio backend so playback can be tested without a
/// device, and swapped for a real implementation in the daemon.
pub trait AudioSink {
    type Error: std::error::Error + Send + Sync + 'static;

    fn play(&mut self, track: &Track, path: &Path) -> Result<(), Self::Error>;
    fn pause(&mut self) -> Result<(), Self::Error>;
    fn resume(&mut self) -> Result<(), Self::Error>;
    fn stop(&mut self) -> Result<(), Self::Error>;
    fn seek(&mut self, position: Duration) -> Result<(), Self::Error>;
    fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error>;
}

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("audio sink error: {0}")]
    Sink(String),
    #[error("no current track")]
    NoCurrentTrack,
    #[error("queue index out of bounds")]
    IndexOutOfBounds,
}

/// Headless playback service. Owns the queue, playback state and event bus.
pub struct PlaybackService<S: AudioSink> {
    queue: Queue,
    status: PlaybackStatus,
    position: Duration,
    volume: u8,
    sink: S,
    cache_dir: PathBuf,
    events: EventBus,
}

impl<S: AudioSink> PlaybackService<S> {
    pub fn new(sink: S, cache_dir: impl Into<PathBuf>) -> Self {
        let queue = Queue::new();
        let events = EventBus::new(Snapshot {
            playback: PlaybackSnapshot::default(),
            queue: queue.snapshot(),
        });
        Self {
            queue,
            status: PlaybackStatus::Stopped,
            position: Duration::ZERO,
            volume: 50,
            sink,
            cache_dir: cache_dir.into(),
            events,
        }
    }

    pub fn events(&self) -> EventBus {
        self.events.clone()
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        PlaybackSnapshot {
            status: self.status,
            current_index: self.queue.current_index(),
            current: self.queue.current_track().cloned(),
            position_secs: self.position.as_secs_f64(),
            volume: self.volume,
            repeat: self.queue.repeat(),
            shuffle: self.queue.shuffle(),
        }
    }

    pub fn queue_snapshot(&self) -> crate::model::QueueSnapshot {
        self.queue.snapshot()
    }

    pub fn play_playlist(&mut self, tracks: Vec<Track>) -> Result<(), PlaybackError> {
        self.queue.replace(tracks);
        self.start_current()
    }

    pub fn play_track(&mut self, track: Track) -> Result<(), PlaybackError> {
        self.queue.play_now(track);
        self.start_current()
    }

    pub fn enqueue(&mut self, track: Track) {
        self.queue.enqueue(track);
        self.publish_queue();
    }

    pub fn remove(&mut self, index: usize) -> Result<(), PlaybackError> {
        let was_current = self.queue.current_index() == Some(index);
        self.queue.remove(index);
        if was_current {
            self.start_current()
        } else {
            self.publish_all();
            Ok(())
        }
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        let moved = self.queue.move_item(from, to);
        if moved {
            self.publish_all();
        }
        moved
    }

    pub fn clear(&mut self) -> Result<(), PlaybackError> {
        let _ = self.sink.stop();
        self.queue.clear();
        self.status = PlaybackStatus::Stopped;
        self.position = Duration::ZERO;
        self.publish_all();
        Ok(())
    }

    pub fn play_index(&mut self, index: usize) -> Result<(), PlaybackError> {
        if !self.queue.play_index(index) {
            return Err(PlaybackError::IndexOutOfBounds);
        }
        self.start_current()
    }

    pub fn pause(&mut self) -> Result<(), PlaybackError> {
        if self.status == PlaybackStatus::Playing {
            self.sink
                .pause()
                .map_err(|error| PlaybackError::Sink(error.to_string()))?;
            self.status = PlaybackStatus::Paused;
            self.publish_playback();
        }
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), PlaybackError> {
        if self.status == PlaybackStatus::Paused {
            self.sink
                .resume()
                .map_err(|error| PlaybackError::Sink(error.to_string()))?;
            self.status = PlaybackStatus::Playing;
            self.publish_playback();
        }
        Ok(())
    }

    pub fn toggle(&mut self) -> Result<(), PlaybackError> {
        match self.status {
            PlaybackStatus::Playing => self.pause(),
            PlaybackStatus::Paused => self.resume(),
            PlaybackStatus::Stopped => self.start_current(),
        }
    }

    pub fn next_track(&mut self) -> Result<(), PlaybackError> {
        match self.queue.next_index() {
            Some(index) => {
                self.queue.set_current(Some(index));
                self.start_current()
            }
            None => {
                self.stop_playback();
                Ok(())
            }
        }
    }

    pub fn previous_track(&mut self) -> Result<(), PlaybackError> {
        match self.queue.previous_index() {
            Some(index) => {
                self.queue.set_current(Some(index));
                self.start_current()
            }
            None => {
                self.stop_playback();
                Ok(())
            }
        }
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), PlaybackError> {
        self.sink
            .seek(position)
            .map_err(|error| PlaybackError::Sink(error.to_string()))?;
        self.position = position;
        self.publish_playback();
        Ok(())
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<(), PlaybackError> {
        let volume = volume.min(100);
        self.sink
            .set_volume(volume)
            .map_err(|error| PlaybackError::Sink(error.to_string()))?;
        self.volume = volume;
        self.publish_playback();
        Ok(())
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.queue.set_repeat(repeat);
        self.publish_all();
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        let seed = fastrand::u64(..);
        self.set_shuffle_with_seed(enabled, seed);
    }

    pub fn set_shuffle_with_seed(&mut self, enabled: bool, seed: u64) {
        self.queue.set_shuffle_with_seed(enabled, seed);
        self.publish_all();
    }

    pub fn on_track_finished(&mut self) -> Result<(), PlaybackError> {
        if let Some(track) = self.queue.current_track() {
            self.events.publish(CoreEvent::TrackEnded {
                video_id: track.video_id.clone(),
            });
        }
        if self.queue.repeat() == RepeatMode::One {
            self.start_current()
        } else {
            self.next_track()
        }
    }

    pub fn on_download_failed(&mut self, video_id: &str) {
        self.events.publish(CoreEvent::Error {
            message: format!("download failed for {video_id}"),
        });
        if self
            .queue
            .current_track()
            .is_some_and(|track| track.video_id == video_id)
        {
            let _ = self.next_track();
        }
    }

    pub fn set_download_status(
        &mut self,
        video_id: &str,
        status: common_structs::MusicDownloadStatus,
    ) {
        self.queue.set_download_status(video_id, status);
        self.events.publish(CoreEvent::DownloadStatus {
            video_id: video_id.to_string(),
            status,
        });
        self.publish_queue();
    }

    fn start_current(&mut self) -> Result<(), PlaybackError> {
        let Some(track) = self.queue.current_track().cloned() else {
            self.stop_playback();
            return Ok(());
        };

        let path = self
            .cache_dir
            .join("downloads")
            .join(format!("{}.mp4", track.video_id));
        if let Err(error) = self.sink.play(&track, &path) {
            self.status = PlaybackStatus::Stopped;
            self.position = Duration::ZERO;
            let message = error.to_string();
            self.events.publish(CoreEvent::Error {
                message: message.clone(),
            });
            self.publish_all();
            return Err(PlaybackError::Sink(message));
        }

        self.status = PlaybackStatus::Playing;
        self.position = Duration::ZERO;
        self.sink
            .set_volume(self.volume)
            .map_err(|error| PlaybackError::Sink(error.to_string()))?;
        self.publish_all();
        Ok(())
    }

    fn stop_playback(&mut self) {
        let _ = self.sink.stop();
        self.status = PlaybackStatus::Stopped;
        self.position = Duration::ZERO;
        self.publish_all();
    }

    fn publish_playback(&self) {
        self.events.publish(CoreEvent::Playback(self.snapshot()));
    }

    fn publish_queue(&self) {
        self.events.publish(CoreEvent::Queue(self.queue.snapshot()));
    }

    fn publish_all(&self) {
        self.publish_playback();
        self.publish_queue();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CoreEvent;
    use common_structs::MusicDownloadStatus;
    use std::path::PathBuf;

    #[derive(Default)]
    struct MockSink {
        plays: Vec<(String, PathBuf)>,
        actions: Vec<&'static str>,
        volume: u8,
    }

    impl AudioSink for MockSink {
        type Error = std::io::Error;

        fn play(&mut self, track: &Track, path: &Path) -> Result<(), Self::Error> {
            self.plays
                .push((track.video_id.clone(), path.to_path_buf()));
            self.actions.push("play");
            Ok(())
        }

        fn pause(&mut self) -> Result<(), Self::Error> {
            self.actions.push("pause");
            Ok(())
        }

        fn resume(&mut self) -> Result<(), Self::Error> {
            self.actions.push("resume");
            Ok(())
        }

        fn stop(&mut self) -> Result<(), Self::Error> {
            self.actions.push("stop");
            Ok(())
        }

        fn seek(&mut self, _position: Duration) -> Result<(), Self::Error> {
            self.actions.push("seek");
            Ok(())
        }

        fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
            self.volume = volume;
            self.actions.push("volume");
            Ok(())
        }
    }

    fn tracks(count: usize) -> Vec<Track> {
        (0..count)
            .map(|index| Track {
                video_id: format!("video-{index}"),
                title: format!("Track {index}"),
                author: "Artist".to_string(),
                album: "Album".to_string(),
                duration: "3:00".to_string(),
            })
            .collect()
    }

    fn service() -> PlaybackService<MockSink> {
        PlaybackService::new(MockSink::default(), "/tmp/ytermusicd-test")
    }

    #[test]
    fn play_playlist_starts_first_track() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        let snapshot = service.snapshot();
        assert_eq!(snapshot.status, PlaybackStatus::Playing);
        assert_eq!(snapshot.current.unwrap().video_id, "video-0");
        assert_eq!(service.queue_snapshot().current, Some(0));
        assert_eq!(
            service.events.snapshot().playback.status,
            PlaybackStatus::Playing
        );
    }

    #[test]
    fn pause_resume_and_toggle() {
        let mut service = service();
        service.play_playlist(tracks(1)).unwrap();
        service.pause().unwrap();
        assert_eq!(service.snapshot().status, PlaybackStatus::Paused);
        service.resume().unwrap();
        assert_eq!(service.snapshot().status, PlaybackStatus::Playing);
        service.toggle().unwrap();
        assert_eq!(service.snapshot().status, PlaybackStatus::Paused);
    }

    #[test]
    fn next_previous_and_stop() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        service.next_track().unwrap();
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-1");
        service.next_track().unwrap();
        assert_eq!(service.snapshot().status, PlaybackStatus::Stopped);
        service.previous_track().unwrap();
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-0");
    }

    #[test]
    fn repeat_all_wraps() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        service.set_repeat(RepeatMode::All);
        service.next_track().unwrap();
        service.next_track().unwrap();
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-0");
    }

    #[test]
    fn repeat_one_restarts_on_finish() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        service.set_repeat(RepeatMode::One);
        service.on_track_finished().unwrap();
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-0");
        assert_eq!(service.sink.plays.len(), 2);
    }

    #[test]
    fn finish_advances_without_repeat() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        service.on_track_finished().unwrap();
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-1");
    }

    #[test]
    fn shuffle_preserves_current_and_restores() {
        let mut service = service();
        service.play_playlist(tracks(6)).unwrap();
        service.play_index(2).unwrap();
        let current = service.snapshot().current.unwrap().video_id;
        let original: Vec<_> = service
            .queue_snapshot()
            .items
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();
        service.set_shuffle_with_seed(true, 42);
        assert_eq!(service.snapshot().current.unwrap().video_id, current);
        let shuffled: Vec<_> = service
            .queue_snapshot()
            .items
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();
        assert_ne!(original, shuffled);
        service.set_shuffle(false);
        let restored: Vec<_> = service
            .queue_snapshot()
            .items
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();
        assert_eq!(original, restored);
    }

    #[test]
    fn download_failure_skips_current() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        service.on_download_failed("video-0");
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-1");
    }

    #[test]
    fn remove_current_starts_next_or_stops() {
        let mut service = service();
        service.play_playlist(tracks(2)).unwrap();
        service.remove(0).unwrap();
        assert_eq!(service.snapshot().current.unwrap().video_id, "video-1");
        service.remove(0).unwrap();
        assert_eq!(service.snapshot().status, PlaybackStatus::Stopped);
    }

    #[test]
    fn volume_is_clamped_and_applied() {
        let mut service = service();
        service.set_volume(150).unwrap();
        assert_eq!(service.snapshot().volume, 100);
        assert_eq!(service.sink.volume, 100);
    }

    #[test]
    fn download_status_is_published() {
        let mut service = service();
        service.play_playlist(tracks(1)).unwrap();
        let (_, mut events) = service.events().subscribe();
        service.set_download_status("video-0", MusicDownloadStatus::Downloaded);
        let event = events.try_recv().unwrap();
        assert!(matches!(event.event, CoreEvent::DownloadStatus { .. }));
        assert_eq!(
            service.queue_snapshot().items[0].download,
            MusicDownloadStatus::Downloaded
        );
    }

    #[test]
    fn empty_queue_is_safe() {
        let mut service = service();
        service.next_track().unwrap();
        service.previous_track().unwrap();
        service.on_track_finished().unwrap();
        assert_eq!(service.snapshot().status, PlaybackStatus::Stopped);
        assert!(service.snapshot().current.is_none());
    }
}
