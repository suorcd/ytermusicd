use common_structs::MusicDownloadStatus;
use serde::{Deserialize, Serialize};

/// A playable track independent of the YouTube Music API client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub album: String,
    pub duration: String,
}

impl From<ytpapi2::YoutubeMusicVideoRef> for Track {
    fn from(value: ytpapi2::YoutubeMusicVideoRef) -> Self {
        Self {
            video_id: value.video_id,
            title: value.title,
            author: value.author,
            album: value.album,
            duration: value.duration,
        }
    }
}

impl From<&ytpapi2::YoutubeMusicVideoRef> for Track {
    fn from(value: &ytpapi2::YoutubeMusicVideoRef) -> Self {
        Self {
            video_id: value.video_id.clone(),
            title: value.title.clone(),
            author: value.author.clone(),
            album: value.album.clone(),
            duration: value.duration.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    pub track: Track,
    pub download: MusicDownloadStatus,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
    pub current: Option<usize>,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaybackSnapshot {
    pub status: PlaybackStatus,
    pub current_index: Option<usize>,
    pub current: Option<Track>,
    pub position_secs: f64,
    pub volume: u8,
    pub repeat: RepeatMode,
    pub shuffle: bool,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            current_index: None,
            current: None,
            position_secs: 0.0,
            volume: 50,
            repeat: RepeatMode::Off,
            shuffle: false,
        }
    }
}

/// Queue state. When shuffle is enabled the items are reordered in place and
/// the original order is retained so disabling shuffle restores it.
#[derive(Debug, Clone)]
pub struct Queue {
    items: Vec<QueueItem>,
    original: Option<Vec<QueueItem>>,
    current: Option<usize>,
    repeat: RepeatMode,
    shuffle: bool,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            original: None,
            current: None,
            repeat: RepeatMode::Off,
            shuffle: false,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current_item(&self) -> Option<&QueueItem> {
        self.current.and_then(|index| self.items.get(index))
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.current_item().map(|item| &item.track)
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Enable/disable shuffle with a system-generated seed.
    pub fn set_shuffle(&mut self, enabled: bool) {
        let seed = fastrand::u64(..);
        self.set_shuffle_with_seed(enabled, seed);
    }

    /// Deterministic variant used by tests.
    pub fn set_shuffle_with_seed(&mut self, enabled: bool, seed: u64) {
        if enabled == self.shuffle {
            return;
        }
        let current_id = self.current_track().map(|track| track.video_id.clone());

        if enabled {
            self.original = Some(self.items.clone());
            shuffle_items(&mut self.items, seed);
            self.shuffle = true;
        } else {
            if let Some(original) = self.original.take() {
                self.items = original;
            }
            self.shuffle = false;
        }

        self.current = current_id
            .and_then(|id| self.items.iter().position(|item| item.track.video_id == id))
            .or_else(|| (!self.items.is_empty()).then_some(0));
    }

    pub fn replace(&mut self, tracks: Vec<Track>) {
        self.items = tracks
            .into_iter()
            .map(|track| QueueItem {
                track,
                download: MusicDownloadStatus::NotDownloaded,
            })
            .collect();
        self.original = None;
        self.current = (!self.items.is_empty()).then_some(0);

        if self.shuffle && !self.items.is_empty() {
            let seed = fastrand::u64(..);
            self.original = Some(self.items.clone());
            let current_id = self.current_track().map(|track| track.video_id.clone());
            shuffle_items(&mut self.items, seed);
            self.current = current_id
                .and_then(|id| self.items.iter().position(|item| item.track.video_id == id))
                .or_else(|| (!self.items.is_empty()).then_some(0));
        }
    }

    pub fn enqueue(&mut self, track: Track) {
        let item = QueueItem {
            track,
            download: MusicDownloadStatus::NotDownloaded,
        };
        self.items.push(item.clone());
        if let Some(original) = &mut self.original {
            original.push(item);
        }
        if self.current.is_none() {
            self.current = Some(0);
        }
    }

    /// Insert a track after the current track and make it current.
    pub fn play_now(&mut self, track: Track) {
        let item = QueueItem {
            track,
            download: MusicDownloadStatus::NotDownloaded,
        };
        let index = match self.current {
            Some(current) => current + 1,
            None => 0,
        };
        self.items.insert(index, item.clone());
        if let Some(original) = &mut self.original {
            original.push(item);
        }
        self.current = Some(index);
    }

    pub fn play_index(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.current = Some(index);
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<QueueItem> {
        if index >= self.items.len() {
            return None;
        }
        let removed = self.items.remove(index);
        if let Some(original) = &mut self.original {
            original.retain(|item| item.track.video_id != removed.track.video_id);
        }
        self.current = match self.current {
            None => None,
            Some(current) if current == index => {
                if self.items.is_empty() {
                    None
                } else {
                    Some(index.min(self.items.len() - 1))
                }
            }
            Some(current) if current > index => Some(current - 1),
            current => current,
        };
        Some(removed)
    }

    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        let item = self.items.remove(from);
        let track_id = item.track.video_id.clone();
        self.items.insert(to, item);

        if let Some(original) = &mut self.original
            && let Some(original_index) = original
                .iter()
                .position(|entry| entry.track.video_id == track_id)
        {
            let moved = original.remove(original_index);
            let target = to.min(original.len());
            original.insert(target, moved);
        }

        self.current = match self.current {
            Some(current) if current == from => Some(to),
            Some(current) if from < current && to >= current => Some(current - 1),
            Some(current) if from > current && to <= current => Some(current + 1),
            current => current,
        };
        true
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.original = None;
        self.current = None;
    }

    pub fn set_current(&mut self, index: Option<usize>) {
        self.current = index.filter(|index| *index < self.items.len());
    }

    /// Next index for an explicit next/previous command.
    pub fn next_index(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let current = self.current.unwrap_or(0);
        if current + 1 < self.items.len() {
            Some(current + 1)
        } else if self.repeat == RepeatMode::All {
            Some(0)
        } else {
            None
        }
    }

    pub fn previous_index(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let current = self.current.unwrap_or(0);
        if current > 0 {
            Some(current - 1)
        } else if self.repeat == RepeatMode::All {
            Some(self.items.len() - 1)
        } else {
            Some(0)
        }
    }

    pub fn set_download_status(&mut self, video_id: &str, status: MusicDownloadStatus) {
        for item in &mut self.items {
            if item.track.video_id == video_id {
                item.download = status;
            }
        }
        if let Some(original) = &mut self.original {
            for item in original {
                if item.track.video_id == video_id {
                    item.download = status;
                }
            }
        }
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            current: self.current,
            repeat: self.repeat,
            shuffle: self.shuffle,
        }
    }
}

fn shuffle_items<T>(items: &mut [T], seed: u64) {
    let mut rng = fastrand::Rng::with_seed(seed);
    for i in (1..items.len()).rev() {
        let j = rng.usize(0..=i);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_structs::MusicDownloadStatus;

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

    #[test]
    fn replace_and_navigation() {
        let mut queue = Queue::new();
        queue.replace(tracks(3));
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.next_index(), Some(1));
        queue.set_current(Some(1));
        assert_eq!(queue.next_index(), Some(2));
        queue.set_current(Some(2));
        assert_eq!(queue.next_index(), None);
        assert_eq!(queue.previous_index(), Some(1));
    }

    #[test]
    fn repeat_all_wraps_navigation() {
        let mut queue = Queue::new();
        queue.replace(tracks(2));
        queue.set_current(Some(1));
        queue.set_repeat(RepeatMode::All);
        assert_eq!(queue.next_index(), Some(0));
        assert_eq!(queue.previous_index(), Some(0));
    }

    #[test]
    fn enqueue_and_play_now() {
        let mut queue = Queue::new();
        let tracks = tracks(3);
        queue.enqueue(tracks[0].clone());
        queue.enqueue(tracks[1].clone());
        assert_eq!(queue.len(), 2);
        queue.play_now(tracks[2].clone());
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.current_index(), Some(1));
        assert_eq!(queue.current_track().unwrap().video_id, "video-2");
    }

    #[test]
    fn remove_adjusts_current() {
        let mut queue = Queue::new();
        queue.replace(tracks(3));
        queue.set_current(Some(2));
        queue.remove(0);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.current_index(), Some(1));
        queue.remove(1);
        assert_eq!(queue.current_index(), Some(0));
        queue.remove(0);
        assert_eq!(queue.current_index(), None);
    }

    #[test]
    fn move_item_adjusts_current() {
        let mut queue = Queue::new();
        queue.replace(tracks(3));
        queue.set_current(Some(1));
        assert!(queue.move_item(1, 2));
        assert_eq!(queue.current_index(), Some(2));
        assert_eq!(queue.current_track().unwrap().video_id, "video-1");
    }

    #[test]
    fn shuffle_is_stable_and_restorable() {
        let mut queue = Queue::new();
        queue.replace(tracks(8));
        let original: Vec<_> = queue
            .items()
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();

        queue.set_shuffle_with_seed(true, 42);
        let shuffled: Vec<_> = queue
            .items()
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();
        assert_ne!(original, shuffled);

        queue.set_shuffle(false);
        let restored: Vec<_> = queue
            .items()
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();
        assert_eq!(original, restored);

        queue.set_shuffle_with_seed(true, 42);
        let shuffled_again: Vec<_> = queue
            .items()
            .iter()
            .map(|item| item.track.video_id.clone())
            .collect();
        assert_eq!(shuffled, shuffled_again);
    }

    #[test]
    fn shuffle_preserves_current_track() {
        let mut queue = Queue::new();
        queue.replace(tracks(5));
        queue.set_current(Some(2));
        let current = queue.current_track().unwrap().video_id.clone();
        queue.set_shuffle_with_seed(true, 7);
        assert_eq!(queue.current_track().unwrap().video_id, current);
    }

    #[test]
    fn download_status_updates_items() {
        let mut queue = Queue::new();
        queue.replace(tracks(2));
        queue.set_download_status("video-1", MusicDownloadStatus::Downloaded);
        assert_eq!(queue.items()[1].download, MusicDownloadStatus::Downloaded);
    }
}
