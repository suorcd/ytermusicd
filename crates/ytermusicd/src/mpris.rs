use souvlaki::{MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition, SeekDirection};
use std::{sync::mpsc, thread, time::Duration};
use tokio::sync::mpsc::UnboundedSender;
use ytermusicd_core::{CoreEvent, EventBus, PlaybackCommand, PlaybackSnapshot, PlaybackStatus};

pub struct MprisHandle;

#[cfg(target_os = "linux")]
pub fn spawn(
    commands: UnboundedSender<PlaybackCommand>,
    events: EventBus,
) -> Result<MprisHandle, String> {
    use souvlaki::{MediaControls, PlatformConfig};

    let (ready_tx, ready_rx) = mpsc::channel();
    let (snapshot_tx, snapshot_rx) = mpsc::channel();

    thread::Builder::new()
        .name("ytermusicd-mpris".to_string())
        .spawn(move || {
            let mut controls = match MediaControls::new(PlatformConfig {
                dbus_name: "ytermusic",
                display_name: "ytermusicd",
                hwnd: None,
            }) {
                Ok(controls) => {
                    let _ = ready_tx.send(Ok(()));
                    controls
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(format!("{error:?}")));
                    return;
                }
            };

            let _ = controls.attach(move |event| {
                if let Some(command) = map_event(event) {
                    let _ = commands.send(command);
                }
            });

            while let Ok(snapshot) = snapshot_rx.recv() {
                update_controls(&mut controls, &snapshot);
            }
        })
        .map_err(|error| error.to_string())?;

    ready_rx.recv().map_err(|error| error.to_string())??;

    let (initial, mut receiver) = events.subscribe();
    let _ = snapshot_tx.send(initial.playback);
    tokio::spawn(async move {
        while let Ok(envelope) = receiver.recv().await {
            if let CoreEvent::Playback(snapshot) = envelope.event {
                let _ = snapshot_tx.send(snapshot);
            }
        }
    });

    Ok(MprisHandle)
}

#[cfg(not(target_os = "linux"))]
pub fn spawn(
    _commands: UnboundedSender<PlaybackCommand>,
    _events: EventBus,
) -> Result<MprisHandle, String> {
    Ok(MprisHandle)
}

fn update_controls(controls: &mut souvlaki::MediaControls, snapshot: &PlaybackSnapshot) {
    let metadata = media_metadata(snapshot);
    if let Err(error) = controls.set_metadata(metadata) {
        log::warn!("MPRIS metadata update failed: {error:?}");
    }
    let playback = media_playback(snapshot);
    if let Err(error) = controls.set_playback(playback) {
        log::warn!("MPRIS playback update failed: {error:?}");
    }
}

pub fn media_metadata(snapshot: &PlaybackSnapshot) -> MediaMetadata<'_> {
    let current = snapshot.current.as_ref();
    let duration = current
        .and_then(|track| parse_duration(&track.duration))
        .map(Duration::from_secs);
    MediaMetadata {
        title: current.map(|track| track.title.as_str()),
        album: current.map(|track| track.album.as_str()),
        artist: current.map(|track| track.author.as_str()),
        cover_url: None,
        duration,
    }
}

pub fn media_playback(snapshot: &PlaybackSnapshot) -> MediaPlayback {
    let progress = Some(MediaPosition(Duration::from_secs_f64(
        snapshot.position_secs.max(0.0),
    )));
    match snapshot.status {
        PlaybackStatus::Stopped => MediaPlayback::Stopped,
        PlaybackStatus::Playing => MediaPlayback::Playing { progress },
        PlaybackStatus::Paused => MediaPlayback::Paused { progress },
    }
}

pub fn map_event(event: MediaControlEvent) -> Option<PlaybackCommand> {
    match event {
        MediaControlEvent::Toggle => Some(PlaybackCommand::Toggle),
        MediaControlEvent::Play => Some(PlaybackCommand::Resume),
        MediaControlEvent::Pause => Some(PlaybackCommand::Pause),
        MediaControlEvent::Next => Some(PlaybackCommand::Next),
        MediaControlEvent::Previous => Some(PlaybackCommand::Previous),
        MediaControlEvent::Stop => Some(PlaybackCommand::Stop),
        MediaControlEvent::Seek(SeekDirection::Forward) => Some(PlaybackCommand::SeekBy(5.0)),
        MediaControlEvent::Seek(SeekDirection::Backward) => Some(PlaybackCommand::SeekBy(-5.0)),
        MediaControlEvent::SeekBy(SeekDirection::Forward, amount) => {
            Some(PlaybackCommand::SeekBy(amount.as_secs_f64()))
        }
        MediaControlEvent::SeekBy(SeekDirection::Backward, amount) => {
            Some(PlaybackCommand::SeekBy(-amount.as_secs_f64()))
        }
        MediaControlEvent::SetPosition(position) => Some(PlaybackCommand::Seek(position.0)),
        MediaControlEvent::SetVolume(volume) => Some(PlaybackCommand::SetVolume(
            (volume * 100.0).round().clamp(0.0, 100.0) as u8,
        )),
        MediaControlEvent::OpenUri(uri) => {
            log::info!("MPRIS OpenUri ignored: {uri}");
            None
        }
        MediaControlEvent::Raise => None,
        MediaControlEvent::Quit => Some(PlaybackCommand::Stop),
    }
}

fn parse_duration(value: &str) -> Option<u64> {
    let parts: Vec<_> = value.split(':').collect();
    let seconds = match parts.as_slice() {
        [minutes, seconds] => minutes.parse::<u64>().ok()? * 60 + seconds.parse::<u64>().ok()?,
        [hours, minutes, seconds] => {
            hours.parse::<u64>().ok()? * 3600
                + minutes.parse::<u64>().ok()? * 60
                + seconds.parse::<u64>().ok()?
        }
        _ => return None,
    };
    Some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ytermusicd_core::{RepeatMode, Track};

    fn snapshot(status: PlaybackStatus) -> PlaybackSnapshot {
        PlaybackSnapshot {
            status,
            current_index: Some(0),
            current: Some(Track {
                video_id: "v1".to_string(),
                title: "Song".to_string(),
                author: "Artist".to_string(),
                album: "Album".to_string(),
                duration: "3:05".to_string(),
            }),
            position_secs: 12.0,
            volume: 50,
            repeat: RepeatMode::Off,
            shuffle: false,
        }
    }

    #[test]
    fn metadata_comes_from_the_snapshot() {
        let snapshot = snapshot(PlaybackStatus::Playing);
        let metadata = media_metadata(&snapshot);
        assert_eq!(metadata.title, Some("Song"));
        assert_eq!(metadata.artist, Some("Artist"));
        assert_eq!(metadata.album, Some("Album"));
        assert_eq!(metadata.duration, Some(Duration::from_secs(185)));
    }

    #[test]
    fn playback_state_maps() {
        assert!(matches!(
            media_playback(&snapshot(PlaybackStatus::Stopped)),
            MediaPlayback::Stopped
        ));
        match media_playback(&snapshot(PlaybackStatus::Paused)) {
            MediaPlayback::Paused {
                progress: Some(MediaPosition(position)),
            } => assert_eq!(position, Duration::from_secs(12)),
            _ => panic!("expected paused progress"),
        }
    }

    #[test]
    fn media_events_map_to_commands() {
        assert!(matches!(
            map_event(MediaControlEvent::Play),
            Some(PlaybackCommand::Resume)
        ));
        assert!(matches!(
            map_event(MediaControlEvent::Next),
            Some(PlaybackCommand::Next)
        ));
        assert!(matches!(
            map_event(MediaControlEvent::SetVolume(0.42)),
            Some(PlaybackCommand::SetVolume(42))
        ));
        assert!(matches!(
            map_event(MediaControlEvent::SeekBy(
                SeekDirection::Backward,
                Duration::from_secs(10)
            )),
            Some(PlaybackCommand::SeekBy(-10.0))
        ));
        assert!(map_event(MediaControlEvent::Raise).is_none());
    }

    #[test]
    fn duration_parser_handles_formats() {
        assert_eq!(parse_duration("3:05"), Some(185));
        assert_eq!(parse_duration("1:02:03"), Some(3723));
        assert_eq!(parse_duration("bad"), None);
    }
}
