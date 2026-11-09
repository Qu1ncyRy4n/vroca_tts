//! Socket transport and lifecycle. Built only with the `transport` feature
//! (DEC-18); everything socket-, lock-, or temp-dir-shaped lives here.
//!
//! - One `vroca.lock` per runtime directory, taken with `try_lock` before any
//!   socket is touched, so two Rust daemons cannot both decide a socket is
//!   stale and unlink each other's (review bug 10, D7). A live non-Rust
//!   owner (the Python daemon) is detected by connecting.
//! - Blocking accept; shutdown wakes each listener by connecting to it
//!   (review bug 9: no busy-poll, real errors reported).
//! - Legacy requests are read until EOF or a quiet gap, capped at
//!   `MAX_REQUEST`; over-limit is an error reply, never truncation (§7.8,
//!   N6, review bug 7).
//! - Teardown removes only the sockets and directory this daemon created,
//!   checked by inode (DEC-9 rule 1, review bug 6). The lock file stays:
//!   removing a lock file reopens the race it exists to close.

use std::fmt;
use std::fs::{self, DirBuilder, File, TryLockError};
use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{unbounded, Receiver, Sender};
use vroca_core::codec::{decode_request, encode_response, Response};
use vroca_core::operations::{Operation, ProtocolError};
use vroca_core::parser::parse_legacy;
use vroca_core::schema::Preferences;

use crate::owner::{new_daemon_id, OwnerClient, OwnerConfig, OwnerHandle};
use crate::player::Player;

/// §7.8, decided 2026-11-08.
pub const MAX_REQUEST: usize = 64 * 1024;
/// Past this the sender is cut off without a reply, as Python's DRAIN_LIMIT.
const DRAIN_LIMIT: usize = 16 * MAX_REQUEST;
const FIRST_READ_TIMEOUT: Duration = Duration::from_secs(5);
/// The legacy protocol's only end-of-request marker besides EOF (N6).
const LEGACY_IDLE_TIMEOUT: Duration = Duration::from_millis(25);
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);
const ACCEPT_WAKE_GRACE: Duration = Duration::from_secs(1);

const LEGACY_SOCKET: &str = "vroca.sock";
const STRUCTURED_SOCKET: &str = "vroca-v1.sock";
const LOCK_FILE: &str = "vroca.lock";
const SNAPSHOT_FILE: &str = "vroca-state.json";
/// Legacy keeps `0666` for compatibility (D11); the structured socket is
/// owner-only (§7.3).
const LEGACY_MODE: u32 = 0o666;
const STRUCTURED_MODE: u32 = 0o600;

pub struct DaemonConfig {
    pub runtime_dir: PathBuf,
}

#[derive(Debug)]
pub enum StartError {
    /// Another daemon holds the lock or answers on the socket.
    AlreadyRunning(PathBuf),
    Io(String),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning(path) => {
                write!(f, "another daemon is already running ({})", path.display())
            }
            Self::Io(message) => f.write_str(message),
        }
    }
}

fn io_error(what: &str, path: &Path, e: &io::Error) -> StartError {
    StartError::Io(format!("{what} {}: {e}", path.display()))
}

/// A path this daemon created, with the identity it had at creation.
struct Owned {
    path: PathBuf,
    dev: u64,
    ino: u64,
}

impl Owned {
    fn record(path: PathBuf) -> Result<Self, StartError> {
        let meta = fs::symlink_metadata(&path).map_err(|e| io_error("stat", &path, &e))?;
        Ok(Self {
            path,
            dev: meta.dev(),
            ino: meta.ino(),
        })
    }

    /// True while the path still names what we created, not a successor's.
    fn still_ours(&self) -> bool {
        fs::symlink_metadata(&self.path)
            .map(|m| m.dev() == self.dev && m.ino() == self.ino)
            .unwrap_or(false)
    }
}

pub struct Daemon {
    running: Arc<AtomicBool>,
    shutdown_tx: Sender<()>,
    shutdown_rx: Receiver<()>,
    accept_threads: Vec<JoinHandle<()>>,
    owner: OwnerHandle,
    sockets: Vec<Owned>,
    temp_dir: Owned,
    _lock: File,
}

impl Daemon {
    pub fn start(config: DaemonConfig, player: Box<dyn Player>) -> Result<Self, StartError> {
        let dir = &config.runtime_dir;
        let lock = take_lock(&dir.join(LOCK_FILE))?;

        let legacy_path = dir.join(LEGACY_SOCKET);
        let structured_path = dir.join(STRUCTURED_SOCKET);
        let legacy = claim_socket(&legacy_path, LEGACY_MODE)?;
        let legacy_owned = Owned::record(legacy_path)?;
        // DEC-3: both paths exist as one unit. If the second fails, remove
        // the first before returning.
        let structured = match claim_socket(&structured_path, STRUCTURED_MODE) {
            Ok(listener) => listener,
            Err(e) => {
                remove_if_ours(&legacy_owned);
                return Err(e);
            }
        };
        let sockets = vec![legacy_owned, Owned::record(structured_path)?];

        let temp_dir = match make_temp_dir(dir) {
            Ok(owned) => owned,
            Err(e) => {
                sockets.iter().for_each(remove_if_ours);
                return Err(e);
            }
        };

        let owner = OwnerHandle::spawn(
            OwnerConfig {
                daemon_id: new_daemon_id(),
                snapshot_path: dir.join(SNAPSHOT_FILE),
                prefs: Preferences::default(),
            },
            player,
        );

        let running = Arc::new(AtomicBool::new(true));
        let (shutdown_tx, shutdown_rx) = unbounded();
        let accept_threads = vec![
            spawn_accept(
                legacy,
                running.clone(),
                owner.client(),
                shutdown_tx.clone(),
                serve_legacy,
            ),
            spawn_accept(
                structured,
                running.clone(),
                owner.client(),
                shutdown_tx.clone(),
                serve_structured,
            ),
        ];

        Ok(Self {
            running,
            shutdown_tx,
            shutdown_rx,
            accept_threads,
            owner,
            sockets,
            temp_dir,
            _lock: lock,
        })
    }

    /// Ask the daemon to stop, from any thread. `quit` does the same.
    pub fn request_shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Block until shutdown is requested, then tear down: stop accepting,
    /// shut the owner and player down, remove owned paths (DEC-9).
    pub fn wait(self) {
        let _ = self.shutdown_rx.recv();
        self.teardown();
    }

    pub fn stop(self) {
        self.request_shutdown();
        self.wait();
    }

    fn teardown(self) {
        self.running.store(false, Ordering::SeqCst);
        for socket in &self.sockets {
            // Wake the blocking accept; it sees `running == false` and exits.
            // If a successor has replaced the path, connecting would reach
            // its listener, not ours, so do not touch it.
            if socket.still_ours() {
                let _ = UnixStream::connect(&socket.path);
            }
        }
        // Bounded: a listener we could not wake is detached rather than
        // allowed to hang shutdown. It holds only its own fd, which closes
        // when the process exits.
        let deadline = Instant::now() + ACCEPT_WAKE_GRACE;
        while Instant::now() < deadline && !self.accept_threads.iter().all(|t| t.is_finished()) {
            thread::sleep(Duration::from_millis(5));
        }
        for accept in self.accept_threads {
            if accept.is_finished() {
                let _ = accept.join();
            }
        }
        self.owner.shutdown();
        self.sockets.iter().for_each(remove_if_ours);
        if self.temp_dir.still_ours() {
            let _ = fs::remove_dir_all(&self.temp_dir.path);
        }
        // `_lock` drops here, releasing the lock for the next daemon.
    }
}

fn take_lock(path: &Path) -> Result<File, StartError> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| io_error("open", path, &e))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(StartError::AlreadyRunning(path.to_path_buf())),
        Err(TryLockError::Error(e)) => Err(io_error("lock", path, &e)),
    }
}

/// Bind, or prove the existing path has no live owner before unlinking it
/// (D7). Only called while holding the lock.
fn claim_socket(path: &Path, mode: u32) -> Result<UnixListener, StartError> {
    let listener = match UnixListener::bind(path) {
        Ok(listener) => listener,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            if UnixStream::connect(path).is_ok() {
                return Err(StartError::AlreadyRunning(path.to_path_buf()));
            }
            fs::remove_file(path).map_err(|e| io_error("remove stale", path, &e))?;
            UnixListener::bind(path).map_err(|e| io_error("bind", path, &e))?
        }
        Err(e) => return Err(io_error("bind", path, &e)),
    };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|e| io_error("chmod", path, &e))?;
    Ok(listener)
}

/// A fresh `0700` directory for rendered audio (DEC-9 rule 4, N2). `create`
/// fails if the name exists, so it is never someone else's directory.
fn make_temp_dir(runtime_dir: &Path) -> Result<Owned, StartError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let path = runtime_dir.join(format!("vroca-{}-{nanos:09}", std::process::id()));
    DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|e| io_error("create", &path, &e))?;
    Owned::record(path)
}

fn remove_if_ours(owned: &Owned) {
    if owned.still_ours() {
        let _ = fs::remove_file(&owned.path);
    }
}

type Serve = fn(UnixStream, &OwnerClient) -> bool;

fn spawn_accept(
    listener: UnixListener,
    running: Arc<AtomicBool>,
    owner: OwnerClient,
    shutdown_tx: Sender<()>,
    serve: Serve,
) -> JoinHandle<()> {
    thread::spawn(move || loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let owner = owner.clone();
                let shutdown_tx = shutdown_tx.clone();
                // DEC-4: one short-lived thread per connection; a panic here
                // ends that thread only.
                thread::spawn(move || {
                    if serve(stream, &owner) {
                        let _ = shutdown_tx.send(());
                    }
                });
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                eprintln!("vroca-daemon: accept failed: {e}");
                thread::sleep(ACCEPT_ERROR_BACKOFF);
            }
        }
    })
}

#[derive(Debug)]
enum ReadError {
    TooLarge,
    /// Sender exceeded `DRAIN_LIMIT`; dropped without a reply.
    Abandoned,
    Io,
}

/// Read until EOF or `idle` passes with no data, keeping at most
/// `MAX_REQUEST` bytes but draining past it so the reply can be delivered.
fn read_request(stream: &mut UnixStream, idle: Duration) -> Result<Vec<u8>, ReadError> {
    stream
        .set_read_timeout(Some(FIRST_READ_TIMEOUT))
        .map_err(|_| ReadError::Io)?;
    let mut kept = Vec::new();
    let mut total = 0usize;
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                total = total.saturating_add(n);
                if total > DRAIN_LIMIT {
                    return Err(ReadError::Abandoned);
                }
                if total <= MAX_REQUEST {
                    if let Some(part) = chunk.get(..n) {
                        kept.extend_from_slice(part);
                    }
                }
                stream
                    .set_read_timeout(Some(idle))
                    .map_err(|_| ReadError::Io)?;
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => break,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(ReadError::Io),
        }
    }
    if total > MAX_REQUEST {
        Err(ReadError::TooLarge)
    } else {
        Ok(kept)
    }
}

fn too_large() -> ProtocolError {
    ProtocolError::invalid_request(format!("request too large (max {MAX_REQUEST} bytes)"))
}

/// Returns true when the request was `quit` and the reply has been sent.
fn serve_legacy(mut stream: UnixStream, owner: &OwnerClient) -> bool {
    let mut quit = false;
    let reply = match read_request(&mut stream, LEGACY_IDLE_TIMEOUT) {
        Err(ReadError::TooLarge) => too_large().message,
        Err(ReadError::Abandoned | ReadError::Io) => return false,
        Ok(bytes) => {
            // Legacy keeps replacement decoding for invalid UTF-8
            // (legacy-compatibility.md transport baseline).
            let text = String::from_utf8_lossy(&bytes);
            match parse_legacy(&text) {
                Err(e) => e.message,
                Ok(op) => {
                    quit = op == Operation::Quit;
                    legacy_reply(owner.request(op))
                }
            }
        }
    };
    let _ = stream.write_all(reply.as_bytes());
    quit
}

fn legacy_reply(result: Result<Response, ProtocolError>) -> String {
    match result {
        Ok(Response::Ok) => "ok".to_string(),
        Ok(Response::String { data }) => data,
        Ok(Response::Status { snapshot }) => {
            serde_json::to_string_pretty(&snapshot).unwrap_or_default()
        }
        Ok(Response::Catalogue { data }) => data.to_string(),
        Ok(Response::Error { error }) | Err(error) => error.message,
    }
}

/// Structured clients half-close after the request, so EOF frames it; the
/// idle timeout only bounds a stalled sender.
fn serve_structured(mut stream: UnixStream, owner: &OwnerClient) -> bool {
    let mut quit = false;
    let response = match read_request(&mut stream, FIRST_READ_TIMEOUT) {
        Err(ReadError::TooLarge) => Response::Error { error: too_large() },
        Err(ReadError::Abandoned | ReadError::Io) => return false,
        Ok(bytes) => match decode_request(&bytes) {
            Err(error) => Response::Error { error },
            Ok(op) => {
                quit = op == Operation::Quit;
                owner
                    .request(op)
                    .unwrap_or_else(|error| Response::Error { error })
            }
        },
    };
    if let Ok(bytes) = encode_response(&response) {
        let _ = stream.write_all(&bytes);
    }
    quit
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::player::{FakePlayer, FakePlayerHandle};
    use std::net::Shutdown;

    fn start(dir: &Path) -> Result<(Daemon, FakePlayerHandle), StartError> {
        let (player, handle) = FakePlayer::new();
        let daemon = Daemon::start(
            DaemonConfig {
                runtime_dir: dir.to_path_buf(),
            },
            Box::new(player),
        )?;
        Ok((daemon, handle))
    }

    fn legacy(dir: &Path, request: &[u8]) -> String {
        let mut stream = UnixStream::connect(dir.join(LEGACY_SOCKET)).unwrap();
        stream.write_all(request).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    }

    fn structured(dir: &Path, request: &[u8]) -> Response {
        let mut stream = UnixStream::connect(dir.join(STRUCTURED_SOCKET)).unwrap();
        stream.write_all(request).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        serde_json::from_slice(&reply).unwrap()
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn d7_second_daemon_refuses_and_first_keeps_serving() {
        let dir = tempfile::tempdir().unwrap();
        let (first, _p) = start(dir.path()).unwrap();
        let second = start(dir.path());
        assert!(matches!(second, Err(StartError::AlreadyRunning(_))));
        // The refusal must not have unlinked or replaced the first's socket.
        assert!(legacy(dir.path(), b"status").contains("\"daemon_id\""));
        first.stop();
    }

    #[test]
    fn d7_live_foreign_socket_is_not_unlinked() {
        // A daemon that does not take our lock (the Python one) but answers.
        let dir = tempfile::tempdir().unwrap();
        let foreign = UnixListener::bind(dir.path().join(LEGACY_SOCKET)).unwrap();
        assert!(matches!(
            start(dir.path()),
            Err(StartError::AlreadyRunning(_))
        ));
        assert!(dir.path().join(LEGACY_SOCKET).exists());
        drop(foreign);
    }

    #[test]
    fn stale_socket_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        drop(UnixListener::bind(dir.path().join(LEGACY_SOCKET)).unwrap());
        assert!(dir.path().join(LEGACY_SOCKET).exists());
        let (daemon, _p) = start(dir.path()).unwrap();
        assert_eq!(legacy(dir.path(), b"stop"), "ok");
        daemon.stop();
    }

    #[test]
    fn dec9_shutdown_removes_only_owned_paths() {
        // Review bug 6: the old cleanup deleted every `vroca-*` entry, and
        // its test passed because of that over-deletion.
        let dir = tempfile::tempdir().unwrap();
        let foreign_dir = dir.path().join("vroca-12345-0");
        let foreign_mpv = dir.path().join("vroca-12345-mpv.sock");
        fs::create_dir(&foreign_dir).unwrap();
        fs::File::create(&foreign_mpv).unwrap();

        let (daemon, _p) = start(dir.path()).unwrap();
        let own_dir = daemon.temp_dir.path.clone();
        assert_eq!(mode(&own_dir), 0o700);
        daemon.stop();

        assert!(!dir.path().join(LEGACY_SOCKET).exists());
        assert!(!dir.path().join(STRUCTURED_SOCKET).exists());
        assert!(!own_dir.exists());
        assert!(foreign_dir.exists(), "foreign temp dir was deleted");
        assert!(foreign_mpv.exists(), "foreign mpv socket was deleted");

        // The lock is released: a successor starts cleanly.
        let (next, _p) = start(dir.path()).unwrap();
        next.stop();
    }

    #[test]
    fn dec9_shutdown_leaves_a_successors_socket_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, _p) = start(dir.path()).unwrap();
        // Something replaced our path while we ran; it is not ours to remove.
        fs::remove_file(dir.path().join(LEGACY_SOCKET)).unwrap();
        let successor = UnixListener::bind(dir.path().join(LEGACY_SOCKET)).unwrap();
        daemon.stop();
        assert!(dir.path().join(LEGACY_SOCKET).exists());
        drop(successor);
    }

    #[test]
    fn n3_quit_replies_bye_then_tears_down() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, player) = start(dir.path()).unwrap();
        assert_eq!(legacy(dir.path(), b"quit"), "bye");
        daemon.wait(); // returns because quit requested shutdown
        assert!(!dir.path().join(LEGACY_SOCKET).exists());
        assert!(!dir.path().join(STRUCTURED_SOCKET).exists());
        assert_eq!(
            player.calls().last(),
            Some(&crate::player::PlayerCall::Shutdown)
        );
    }

    #[test]
    fn dec3_socket_modes() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, _p) = start(dir.path()).unwrap();
        assert_eq!(mode(&dir.path().join(LEGACY_SOCKET)), LEGACY_MODE);
        assert_eq!(mode(&dir.path().join(STRUCTURED_SOCKET)), STRUCTURED_MODE);
        daemon.stop();
    }

    #[test]
    fn n6_framing_and_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, _p) = start(dir.path()).unwrap();
        // Review bug 1 end to end: socat sends a trailing newline.
        assert_eq!(legacy(dir.path(), b"speed 1.5\n"), "ok");
        // Review bug 7: invalid UTF-8 gets a reply, not a hang.
        assert_eq!(legacy(dir.path(), b"say caf\xc3"), "ok");
        assert_eq!(legacy(dir.path(), b""), "unknown: ");
        let mut big = b"say ".to_vec();
        big.resize(MAX_REQUEST + 1, b'x');
        assert_eq!(
            legacy(dir.path(), &big),
            "request too large (max 65536 bytes)"
        );
        let mut at_limit = b"say ".to_vec();
        at_limit.resize(MAX_REQUEST, b'x');
        assert_eq!(legacy(dir.path(), &at_limit), "ok");
        // Still serving after all of it (D8).
        assert!(legacy(dir.path(), b"status").contains("\"speed\": 1.5"));
        daemon.stop();
    }

    #[test]
    fn legacy_errors_use_python_reply_text() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, _p) = start(dir.path()).unwrap();
        assert_eq!(legacy(dir.path(), b"frob x"), "unknown: frob x");
        assert_eq!(
            legacy(dir.path(), b"speed abc"),
            "speed needs a number between 0.5 and 3"
        );
        daemon.stop();
    }

    #[test]
    fn structured_socket_round_trip_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, _p) = start(dir.path()).unwrap();
        match structured(dir.path(), br#"{"v":1,"type":"status"}"#) {
            Response::Status { snapshot } => assert_eq!(snapshot.schema, 1),
            other => panic!("{other:?}"),
        }
        // DEC-19: an unversioned or other-version request is rejected.
        for request in [&br#"{"type":"status"}"#[..], br#"{"v":2,"type":"status"}"#] {
            match structured(dir.path(), request) {
                Response::Error { error } => assert_eq!(
                    error.code,
                    vroca_core::operations::ErrorCode::InvalidRequest
                ),
                other => panic!("{other:?}"),
            }
        }
        // DEC-3: a legacy client on the structured path gets a typed error.
        match structured(dir.path(), b"status") {
            Response::Error { error } => {
                assert_eq!(
                    error.code,
                    vroca_core::operations::ErrorCode::InvalidRequest
                )
            }
            other => panic!("{other:?}"),
        }
        daemon.stop();
    }

    #[test]
    fn n4_player_death_visible_over_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let (daemon, player) = start(dir.path()).unwrap();
        player.kill("mpv exited");
        assert_eq!(legacy(dir.path(), b"toggle"), "ok");
        let status = legacy(dir.path(), b"status");
        assert!(status.contains("\"state\": \"degraded\""), "{status}");
        assert!(status.contains("player: mpv exited"), "{status}");
        daemon.stop();
    }
}
