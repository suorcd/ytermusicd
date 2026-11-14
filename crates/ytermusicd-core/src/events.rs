use crate::model::{PlaybackSnapshot, QueueSnapshot};
use common_structs::MusicDownloadStatus;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::broadcast;

/// Number of events kept for `Last-Event-ID` replay. Matches the broadcast
/// channel capacity so a client that can replay is never silently dropped.
pub const EVENT_HISTORY_CAPACITY: usize = 256;

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
/// event ids. Recent events are retained so SSE clients can resume with
/// `Last-Event-ID`; if a client falls further behind than the retained history
/// it must resynchronise from a fresh snapshot.
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<Inner>,
}

struct Inner {
    next_id: AtomicU64,
    tx: broadcast::Sender<EventEnvelope>,
    snapshot: RwLock<Snapshot>,
    history: Mutex<VecDeque<EventEnvelope>>,
}

/// A subscription with optional replay data.
pub struct Subscription {
    /// Current full state. Use it when `needs_snapshot` is true.
    pub snapshot: Snapshot,
    /// Events after the requested `Last-Event-ID`, oldest first.
    pub replay: Vec<EventEnvelope>,
    /// True for a fresh connection or when the requested id is no longer in
    /// the replay window.
    pub needs_snapshot: bool,
    /// Newest id published at subscription time (0 when no events exist).
    pub latest_id: u64,
    pub receiver: broadcast::Receiver<EventEnvelope>,
}

impl EventBus {
    pub fn new(snapshot: Snapshot) -> Self {
        let (tx, _) = broadcast::channel(EVENT_HISTORY_CAPACITY);
        Self {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(1),
                tx,
                snapshot: RwLock::new(snapshot),
                history: Mutex::new(VecDeque::with_capacity(EVENT_HISTORY_CAPACITY)),
            }),
        }
    }

    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<EventEnvelope>) {
        (self.snapshot(), self.inner.tx.subscribe())
    }

    /// Subscribe for an SSE client, replaying retained events after
    /// `last_id`. Subscribe before reading history: an event published between
    /// those steps may appear twice, and consumers must skip ids they already
    /// sent. It can never be lost.
    pub fn subscribe_since(&self, last_id: Option<u64>) -> Subscription {
        let receiver = self.inner.tx.subscribe();
        let snapshot = self.snapshot();
        let latest_id = self.inner.next_id.load(Ordering::SeqCst).saturating_sub(1);
        let history: VecDeque<EventEnvelope> = self.inner.history.lock().unwrap().clone();

        let (needs_snapshot, replay) = match last_id {
            None => (true, Vec::new()),
            Some(id) if id > latest_id => (true, Vec::new()),
            Some(id) => match history.front() {
                Some(oldest) if id.saturating_add(1) < oldest.id => (true, Vec::new()),
                _ => (
                    false,
                    history.into_iter().filter(|event| event.id > id).collect(),
                ),
            },
        };

        Subscription {
            snapshot,
            replay,
            needs_snapshot,
            latest_id,
            receiver,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.snapshot.read().unwrap().clone()
    }

    pub fn publish(&self, event: CoreEvent) -> u64 {
        // Serialise id assignment, snapshot updates and history writes so
        // subscribers always observe events in id order.
        let mut history = self.inner.history.lock().unwrap();
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        {
            let mut snapshot = self.inner.snapshot.write().unwrap();
            match &event {
                CoreEvent::Playback(playback) => snapshot.playback = playback.clone(),
                CoreEvent::Queue(queue) => snapshot.queue = queue.clone(),
                _ => {}
            }
        }
        let envelope = EventEnvelope { id, event };
        if history.len() == EVENT_HISTORY_CAPACITY {
            history.pop_front();
        }
        history.push_back(envelope.clone());
        let _ = self.inner.tx.send(envelope);
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

    #[test]
    fn subscribe_since_replays_missed_events() {
        let bus = EventBus::new(Snapshot::default());
        let first = bus.publish(playback_event());
        let second = bus.publish(CoreEvent::LibraryChanged);

        let subscription = bus.subscribe_since(Some(first));
        assert!(!subscription.needs_snapshot);
        assert_eq!(subscription.latest_id, second);
        assert_eq!(subscription.replay.len(), 1);
        assert_eq!(subscription.replay[0].id, second);
        assert_eq!(
            subscription.snapshot.playback.status,
            PlaybackStatus::Playing
        );
    }

    #[test]
    fn subscribe_since_can_replay_from_zero() {
        let bus = EventBus::new(Snapshot::default());
        bus.publish(CoreEvent::LibraryChanged);
        let subscription = bus.subscribe_since(Some(0));
        assert!(!subscription.needs_snapshot);
        assert_eq!(subscription.replay.len(), 1);
    }

    #[test]
    fn fresh_subscribe_requests_snapshot() {
        let bus = EventBus::new(Snapshot::default());
        let subscription = bus.subscribe_since(None);
        assert!(subscription.needs_snapshot);
        assert!(subscription.replay.is_empty());
        assert_eq!(subscription.latest_id, 0);
    }

    #[test]
    fn evicted_history_requests_snapshot() {
        let bus = EventBus::new(Snapshot::default());
        for _ in 0..=EVENT_HISTORY_CAPACITY {
            bus.publish(CoreEvent::LibraryChanged);
        }

        let stale = bus.subscribe_since(Some(0));
        assert!(stale.needs_snapshot);
        assert!(stale.replay.is_empty());

        let resumable = bus.subscribe_since(Some(1));
        assert!(!resumable.needs_snapshot);
        assert_eq!(resumable.replay.len(), EVENT_HISTORY_CAPACITY);
        assert_eq!(resumable.replay[0].id, 2);
        assert_eq!(
            resumable.replay.last().unwrap().id,
            EVENT_HISTORY_CAPACITY as u64 + 1
        );
    }

    #[test]
    fn future_event_id_requests_snapshot() {
        let bus = EventBus::new(Snapshot::default());
        bus.publish(CoreEvent::LibraryChanged);
        let subscription = bus.subscribe_since(Some(99));
        assert!(subscription.needs_snapshot);
        assert_eq!(subscription.latest_id, 1);
    }
}
