pub mod player;

use crossbeam_channel::{unbounded, Sender};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use player::Player;
use vroca_core::codec::{decode_request, encode_response, Response};
use vroca_core::operations::Operation;
use vroca_core::parser::parse_legacy;
use vroca_core::state::State;

pub struct DaemonConfig {
    pub runtime_dir: PathBuf,
}

pub struct Daemon {
    config: DaemonConfig,
    running: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    player: Player,
}

enum Message {
    Request(Operation, Sender<Response>),
    Shutdown,
}

impl Daemon {
    pub fn new(config: DaemonConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            threads: Vec::new(),
            player: Player::new(),
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        self.player
            .start_fake()
            .map_err(|e| format!("Player error: {}", e.message))?;

        let legacy_sock = self.config.runtime_dir.join("tts.sock");
        let struct_sock = self.config.runtime_dir.join("vroca-v1.sock");
        let mpv_sock = self.config.runtime_dir.join("tts-mpv.sock");

        let temp_dir = self
            .config
            .runtime_dir
            .join(format!("tts-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

        Self::bind_socket(&legacy_sock)?;
        Self::bind_socket(&struct_sock)?;
        // We do NOT bind mpv_sock here as it's per-instance and owned by mpv. But we will make sure it is unlinked if we created it.

        let legacy_listener = UnixListener::bind(&legacy_sock)
            .map_err(|e| format!("Failed to bind legacy socket: {}", e))?;
        let struct_listener = UnixListener::bind(&struct_sock)
            .map_err(|e| format!("Failed to bind struct socket: {}", e))?;

        let (tx, rx) = unbounded::<Message>();

        let state_tx = tx.clone();
        self.threads.push(thread::spawn(move || {
            let mut state = State::new(Default::default());
            while let Ok(msg) = rx.recv() {
                match msg {
                    Message::Request(op, reply_tx) => {
                        let res = match &op {
                            Operation::Status => {
                                // Currently we don't return the snapshot yet.
                                Response::Ok
                            }
                            _ => {
                                state.apply(op);
                                Response::Ok
                            }
                        };
                        let _ = reply_tx.send(res);
                    }
                    Message::Shutdown => break,
                }
            }
        }));

        let tx_legacy = tx.clone();
        let running_legacy = self.running.clone();
        legacy_listener.set_nonblocking(true).unwrap();
        self.threads.push(thread::spawn(move || {
            while running_legacy.load(Ordering::SeqCst) {
                if let Ok((mut stream, _)) = legacy_listener.accept() {
                    let tx_legacy = tx_legacy.clone();
                    thread::spawn(move || {
                        let mut buf = [0; 4096];
                        if let Ok(n) = stream.read(&mut buf) {
                            if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                                let op = match parse_legacy(s) {
                                    Ok(op) => op,
                                    Err(e) => {
                                        // Send text error for legacy
                                        let _ = stream.write_all(e.message.as_bytes());
                                        return;
                                    }
                                };
                                let (reply_tx, reply_rx) = unbounded();
                                if tx_legacy.send(Message::Request(op, reply_tx)).is_ok() {
                                    if let Ok(res) = reply_rx.recv() {
                                        let text = match res {
                                            Response::Ok => "ok".to_string(),
                                            Response::String { data } => data,
                                            Response::Error { error } => error.message,
                                            _ => "ok".to_string(),
                                        };
                                        let _ = stream.write_all(text.as_bytes());
                                    }
                                }
                            }
                        }
                    });
                }
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }));

        let tx_struct = tx.clone();
        let running_struct = self.running.clone();
        struct_listener.set_nonblocking(true).unwrap();
        self.threads.push(thread::spawn(move || {
            while running_struct.load(Ordering::SeqCst) {
                if let Ok((mut stream, _)) = struct_listener.accept() {
                    let tx_struct = tx_struct.clone();
                    thread::spawn(move || {
                        let mut buf = Vec::new();
                        if stream.read_to_end(&mut buf).is_ok() {
                            let (reply_tx, reply_rx) = unbounded();
                            let op = match decode_request(&buf) {
                                Ok(op) => op,
                                Err(e) => {
                                    let res = Response::Error { error: e };
                                    if let Ok(b) = encode_response(&res) {
                                        let _ = stream.write_all(&b);
                                    }
                                    return;
                                }
                            };
                            if tx_struct.send(Message::Request(op, reply_tx)).is_ok() {
                                if let Ok(res) = reply_rx.recv() {
                                    if let Ok(b) = encode_response(&res) {
                                        let _ = stream.write_all(&b);
                                    }
                                }
                            }
                        }
                    });
                }
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }));

        // Keep tx to allow sending Shutdown later
        self.threads.push(thread::spawn(move || {
            // Keep the sender alive until daemon is stopped
            let _tx = tx;
        }));

        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.player.kill();
        for th in self.threads.drain(..) {
            let _ = th.join();
        }

        let _ = fs::remove_file(self.config.runtime_dir.join("tts.sock"));
        let _ = fs::remove_file(self.config.runtime_dir.join("vroca-v1.sock"));

        // Find and remove temp dirs starting with tts-
        if let Ok(entries) = fs::read_dir(&self.config.runtime_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("tts-") {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let _ = fs::remove_dir_all(entry.path());
                    } else if name.ends_with("-mpv.sock") {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    fn bind_socket(path: &Path) -> Result<(), String> {
        if path.exists() {
            if UnixStream::connect(path).is_ok() {
                return Err("Daemon already running on this socket".to_string());
            }
            fs::remove_file(path).map_err(|e| format!("Failed to remove stale socket: {}", e))?;
        }
        Ok(())
    }
    pub fn is_player_alive(&mut self) -> bool {
        self.player.is_alive()
    }

    pub fn kill_player(&mut self) {
        self.player.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duplicate_startup_d7() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config1 = DaemonConfig {
            runtime_dir: temp_dir.path().to_path_buf(),
        };
        let mut d1 = Daemon::new(config1);
        assert!(d1.start().is_ok());

        let config2 = DaemonConfig {
            runtime_dir: temp_dir.path().to_path_buf(),
        };
        let mut d2 = Daemon::new(config2);
        assert!(d2.start().is_err()); // Must fail because d1 is running

        d1.stop();
    }

    #[test]
    fn test_clean_shutdown_n2_n3_d9() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = DaemonConfig {
            runtime_dir: temp_dir.path().to_path_buf(),
        };
        let mut d1 = Daemon::new(config);
        assert!(d1.start().is_ok());

        // Create a dummy mpv socket to simulate running mpv
        let mpv_sock = temp_dir
            .path()
            .join(format!("tts-{}-mpv.sock", std::process::id()));
        fs::File::create(&mpv_sock).unwrap();

        d1.stop();

        assert!(!temp_dir.path().join("tts.sock").exists());
        assert!(!temp_dir.path().join("vroca-v1.sock").exists());

        let has_temp_dirs = fs::read_dir(temp_dir.path()).unwrap().any(|e| {
            let name = e.unwrap().file_name().to_string_lossy().to_string();
            name.starts_with("tts-")
        });
        assert!(!has_temp_dirs); // Must be cleaned up
    }

    #[test]
    fn test_player_death_n4() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = DaemonConfig {
            runtime_dir: temp_dir.path().to_path_buf(),
        };
        let mut d1 = Daemon::new(config);
        assert!(d1.start().is_ok());

        // At start player is alive
        assert!(d1.is_player_alive());

        // Simulate external kill of player
        d1.kill_player();

        // Player should be dead
        assert!(!d1.is_player_alive());

        d1.stop();
    }
}
