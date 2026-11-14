use player::{Player, PlayerOptions};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use ytermusicd_core::{AudioSink, Track};

enum AudioCommand {
    Play(std::path::PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    Volume(u8),
}

/// A rodio sink that lives on a dedicated thread, so the non-Send audio stream
/// never enters the async runtime.
pub struct RodioSink {
    tx: mpsc::Sender<AudioCommand>,
    finished: Arc<AtomicBool>,
}

impl RodioSink {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let finished = Arc::new(AtomicBool::new(true));
        let thread_finished = finished.clone();

        thread::Builder::new()
            .name("ytermusicd-audio".to_string())
            .spawn(move || {
                let (error_tx, _error_rx) = flume::unbounded();
                let mut player = match Player::new(error_tx, PlayerOptions::new(50)) {
                    Ok(player) => player,
                    Err(error) => {
                        log::error!("audio output unavailable: {error:?}");
                        return;
                    }
                };
                let mut playing = false;

                loop {
                    match rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(AudioCommand::Play(path)) => {
                            if let Err(error) = player.play(&path) {
                                log::error!("failed to play {}: {error:?}", path.display());
                                thread_finished.store(true, Ordering::SeqCst);
                                playing = false;
                            } else {
                                thread_finished.store(false, Ordering::SeqCst);
                                playing = true;
                            }
                        }
                        Ok(AudioCommand::Pause) => player.pause(),
                        Ok(AudioCommand::Resume) => player.resume(),
                        Ok(AudioCommand::Stop) => {
                            player.stop();
                            thread_finished.store(true, Ordering::SeqCst);
                            playing = false;
                        }
                        Ok(AudioCommand::Seek(position)) => player.seek_to(position),
                        Ok(AudioCommand::Volume(volume)) => player.set_volume(volume as i32),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if playing && player.is_finished() {
                                thread_finished.store(true, Ordering::SeqCst);
                                playing = false;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .ok();

        Self { tx, finished }
    }

    fn send(&self, command: AudioCommand) -> Result<(), std::io::Error> {
        self.tx
            .send(command)
            .map_err(|_| std::io::Error::other("audio thread is not running"))
    }
}

impl AudioSink for RodioSink {
    type Error = std::io::Error;

    fn play(&mut self, _track: &Track, path: &Path) -> Result<(), Self::Error> {
        self.send(AudioCommand::Play(path.to_path_buf()))
    }

    fn pause(&mut self) -> Result<(), Self::Error> {
        self.send(AudioCommand::Pause)
    }

    fn resume(&mut self) -> Result<(), Self::Error> {
        self.send(AudioCommand::Resume)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.send(AudioCommand::Stop)
    }

    fn seek(&mut self, position: Duration) -> Result<(), Self::Error> {
        self.send(AudioCommand::Seek(position))
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
        self.send(AudioCommand::Volume(volume))
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }
}
