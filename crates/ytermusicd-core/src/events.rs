use crate::model::{PlaybackSnapshot, QueueSnapshot};
use common_structs::MusicDownloadStatus;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum CoreEvent {
    Playback(PlaybackSnapshot),
    Queue(QueueSnapshot),
    TrackEnded {
        video_id: String,
    },
    DownloadStatus {
        video_id: String,
        status: MusicDownloadStatus,
    },
    Error {
        message: String,
    },
    LibraryChanged,
}

#[derive(Debug, Clone)]
pub struct EventEnvelope {
    pub id: u64,
    pub event: CoreEvent,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub playback: PlaybackSnapshot,
    pub queue: QueueSnapshot,
}

/// Broadcast event bus with snapshot-on-subscribe and monotonically increasing
/// event ids. Slow subscribers can lag; reconnecting clients should use a
/// fresh snapshot.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<Inner>,
}

struct Inner {
    next_id: AtomicU64,
    tx: broadcast::Sender<EventEnvelope>,
    snapshot: RwLock<Snapshot>,
}

impl EventBus {
    pub fn new(snapshot: Snapshot) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(1),
                tx,
                snapshot: RwLock::new(snapshot),
            }),
        }
    }

    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<EventEnvelope>) {
        (self.snapshot(), self.inner.tx.subscribe())
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.snapshot.read().unwrap().clone()
    }

    pub fn publish(&self, event: CoreEvent) -> u64 {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        {
            let mut snapshot = self.inner.snapshot.write().unwrap();
            match &event {
                CoreEvent::Playback(playback) => snapshot.playback = playback.clone(),
                CoreEvent::Queue(queue) => snapshot.queue = queue.clone(),
                _ => {}
            }
        }
        let _ = self.inner.tx.send(EventEnvelope { id, event });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PlaybackStatus;
    use tokio::sync::broadcast::error::TryRecvError;

    fn playback_event() -> CoreEvent {
        CoreEvent::Playback(PlaybackSnapshot {
            status: PlaybackStatus::Playing,
            current: None,
            position_secs: 1.0,
            ..PlaybackSnapshot::default()
        })
    }

    #[test]
    fn publish_updates_snapshot_and_delivers() {
        let bus = EventBus::new(Snapshot::default());
        let (snapshot, mut rx) = bus.subscribe();
        assert_eq!(snapshot.playback.status, PlaybackStatus::Stopped);

        let id = bus.publish(playback_event());
        assert_eq!(id, 1);
        assert_eq!(bus.snapshot().playback.status, PlaybackStatus::Playing);

        let envelope = rx.try_recv().unwrap();
        assert_eq!(envelope.id, 1);
        assert!(matches!(envelope.event, CoreEvent::Playback(_)));
        assert_eq!(bus.publish(CoreEvent::LibraryChanged), 2);
    }

    #[test]
    fn snapshot_on_subscribe_is_current() {
        let bus = EventBus::new(Snapshot::default());
        bus.publish(playback_event());
        let (snapshot, _rx) = bus.subscribe();
        assert_eq!(snapshot.playback.status, PlaybackStatus::Playing);
    }

    #[test]
    fn multiple_subscribers_receive_events() {
        let bus = EventBus::new(Snapshot::default());
        let (_snapshot, mut first) = bus.subscribe();
        let (_snapshot, mut second) = bus.subscribe();
        bus.publish(CoreEvent::Error {
            message: "boom".to_string(),
        });
        assert!(matches!(
            first.try_recv().unwrap().event,
            CoreEvent::Error { .. }
        ));
        assert!(matches!(
            second.try_recv().unwrap().event,
            CoreEvent::Error { .. }
        ));
    }

    #[test]
    fn no_subscribers_is_not_an_error() {
        let bus = EventBus::new(Snapshot::default());
        let id = bus.publish(CoreEvent::LibraryChanged);
        assert_eq!(id, 1);
        let (_snapshot, mut rx) = bus.subscribe();
        assert_eq!(rx.try_recv().unwrap_err(), TryRecvError::Empty);
    }
}
