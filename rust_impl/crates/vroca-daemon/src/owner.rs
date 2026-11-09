//! The state owner (DEC-4) and snapshot writer (DEC-8).
//!
//! One thread owns all mutable state and receives typed messages over a
//! channel. After every message that changes the published snapshot it
//! rewrites `vroca-state.json` atomically; a message that changes nothing
//! (such as `status`) writes nothing. Player failures degrade health and
//! never stop control operations (DEC-9 rule 3). No sockets here: this
//! module builds with default features (DEC-18).

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::{unbounded, Receiver, Sender};
use vroca_core::codec::Response;
use vroca_core::operations::{Operation, ProtocolError};
use vroca_core::schema::{Health, Preferences, RuntimeSnapshot};
use vroca_core::state::State;

use crate::player::{Player, PlayerEvent};

pub const SNAPSHOT_SCHEMA: u32 = 1;

pub struct OwnerConfig {
    pub daemon_id: String,
    pub snapshot_path: PathBuf,
    pub prefs: Preferences,
}

enum Message {
    Request(Operation, Sender<Response>),
    Player(PlayerEvent),
    Shutdown,
}

/// Owns the state-owner thread. Dropping it without `shutdown` leaves the
/// thread running until every `OwnerClient` is dropped.
pub struct OwnerHandle {
    tx: Sender<Message>,
    thread: Option<JoinHandle<()>>,
}

/// A cheap, cloneable sender for connection threads.
#[derive(Clone)]
pub struct OwnerClient {
    tx: Sender<Message>,
}

/// `<pid>:<unix-ms>`. Unique per start, and a reader can tell a snapshot left
/// by a dead daemon from a live one by checking the pid (DEC-8).
pub fn new_daemon_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}:{ms}", std::process::id())
}

impl OwnerHandle {
    pub fn spawn(config: OwnerConfig, mut player: Box<dyn Player>) -> Self {
        let (tx, rx) = unbounded::<Message>();
        let events_tx = tx.clone();
        player.attach(Box::new(move |event| {
            let _ = events_tx.send(Message::Player(event));
        }));
        let thread = thread::spawn(move || run(config, player, &rx));
        Self {
            tx,
            thread: Some(thread),
        }
    }

    pub fn client(&self) -> OwnerClient {
        OwnerClient {
            tx: self.tx.clone(),
        }
    }

    /// Send `Shutdown`, wait for the owner to release the player and exit.
    pub fn shutdown(mut self) {
        let _ = self.tx.send(Message::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl OwnerClient {
    pub fn request(&self, op: Operation) -> Result<Response, ProtocolError> {
        let gone = || ProtocolError::unavailable("daemon is shutting down");
        let (reply_tx, reply_rx) = unbounded();
        self.tx
            .send(Message::Request(op, reply_tx))
            .map_err(|_| gone())?;
        reply_rx.recv().map_err(|_| gone())
    }
}

fn run(config: OwnerConfig, mut player: Box<dyn Player>, rx: &Receiver<Message>) {
    let mut state = State::new(config.prefs.clone());
    let mut health = Health::Ok;
    let mut published: Option<RuntimeSnapshot> = None;
    publish(&config, &state, &mut health, &mut published);

    while let Ok(message) = rx.recv() {
        match message {
            Message::Request(op, reply) => {
                let response = handle(
                    &config.daemon_id,
                    &mut state,
                    player.as_mut(),
                    &mut health,
                    op,
                );
                let _ = reply.send(response);
            }
            Message::Player(PlayerEvent::Died { reason }) => {
                health.degrade(format!("player: {reason}"));
            }
            Message::Shutdown => break,
        }
        publish(&config, &state, &mut health, &mut published);
    }
    player.shutdown();
}

fn handle(
    daemon_id: &str,
    state: &mut State,
    player: &mut dyn Player,
    health: &mut Health,
    op: Operation,
) -> Response {
    match op {
        Operation::Status => Response::Status {
            snapshot: Box::new(snapshot(daemon_id, state, health)),
        },
        // The transport tears down after replying; the owner only answers.
        Operation::Quit => Response::String { data: "bye".into() },
        op => {
            let (paused, speed, generation) = (state.paused, state.prefs.speed, state.generation);
            state.apply(op);
            // Derive player calls from the state change rather than per
            // operation, so every path that pauses, retimes, or invalidates
            // audio reaches the player the same way.
            let mut results = Vec::new();
            if state.generation != generation {
                results.push(player.stop());
            }
            if state.paused != paused {
                results.push(player.set_paused(state.paused));
            }
            if state.prefs.speed != speed {
                results.push(player.set_speed(state.prefs.speed));
            }
            for err in results.into_iter().filter_map(Result::err) {
                health.degrade(err.problem());
            }
            Response::Ok
        }
    }
}

pub fn snapshot(daemon_id: &str, state: &State, health: &Health) -> RuntimeSnapshot {
    RuntimeSnapshot {
        schema: SNAPSHOT_SCHEMA,
        daemon_id: daemon_id.to_string(),
        // §7.5 is Open; this keeps today's behavior (current sentence only).
        sentence: state
            .sentences
            .get(state.active_sentence_index)
            .cloned()
            .unwrap_or_default(),
        index: state.active_sentence_index,
        total: state.sentences.len(),
        paused: state.paused,
        speed: state.prefs.speed,
        rendered: 0,
        rendering: None,
        loaded: false,
        voice: None,
        word: 0,
        engine: state.prefs.engine.clone(),
        engines: Vec::new(),
        aligner: state.prefs.aligner,
        queue_len: state.waiting_queue.len(),
        font_size: state.prefs.font_size,
        words_visible: state.prefs.words_visible,
        position: state.prefs.position,
        voices: None,
        last_render_ms: None,
        avg_render_ms: None,
        health: health.clone(),
        unknown_fields: Default::default(),
    }
}

fn publish(
    config: &OwnerConfig,
    state: &State,
    health: &mut Health,
    published: &mut Option<RuntimeSnapshot>,
) {
    let current = snapshot(&config.daemon_id, state, health);
    if published.as_ref() == Some(&current) {
        return;
    }
    if let Err(e) = write_atomic(&config.snapshot_path, &current) {
        health.degrade(format!("snapshot: {e}"));
    }
    *published = Some(current);
}

/// Write beside the target and rename over it, so a reader sees the whole
/// old snapshot or the whole new one (DEC-8, DEC-17 rule 5).
pub fn write_atomic(path: &Path, snapshot: &RuntimeSnapshot) -> Result<(), String> {
    let text = serde_json::to_vec_pretty(snapshot).map_err(|e| e.to_string())?;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp-{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::player::{FakePlayer, FakePlayerHandle, PlayerCall};
    use vroca_core::parser::parse_legacy;

    fn spawn(dir: &Path) -> (OwnerHandle, FakePlayerHandle, PathBuf) {
        let path = dir.join("vroca-state.json");
        let (player, handle) = FakePlayer::new();
        let owner = OwnerHandle::spawn(
            OwnerConfig {
                daemon_id: "test:1".into(),
                snapshot_path: path.clone(),
                prefs: Preferences::default(),
            },
            Box::new(player),
        );
        (owner, handle, path)
    }

    fn status(client: &OwnerClient) -> RuntimeSnapshot {
        match client.request(Operation::Status).unwrap() {
            Response::Status { snapshot } => *snapshot,
            other => panic!("status answered {other:?}"),
        }
    }

    fn read_snapshot(path: &Path) -> RuntimeSnapshot {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn dec8_snapshot_written_at_start_with_daemon_id() {
        let dir = tempfile::tempdir().unwrap();
        let (owner, _player, path) = spawn(dir.path());
        let client = owner.client();
        // A request round trip orders us after the initial publish.
        let live = status(&client);
        let file = read_snapshot(&path);
        assert_eq!(file.daemon_id, "test:1");
        assert_eq!(file, live);
        owner.shutdown();
    }

    #[test]
    fn dec8_snapshot_written_on_change_not_on_status() {
        let dir = tempfile::tempdir().unwrap();
        let (owner, _player, path) = spawn(dir.path());
        let client = owner.client();
        status(&client);
        fs::remove_file(&path).unwrap();
        status(&client);
        assert!(!path.exists(), "status rewrote an unchanged snapshot");
        client.request(parse_legacy("speed 2").unwrap()).unwrap();
        status(&client);
        assert_eq!(read_snapshot(&path).speed, 2.0);
        owner.shutdown();
    }

    #[test]
    fn n4_player_death_degrades_health_and_control_keeps_working() {
        let dir = tempfile::tempdir().unwrap();
        let (owner, player, path) = spawn(dir.path());
        let client = owner.client();
        assert_eq!(status(&client).health, Health::Ok);

        player.kill("exited with status 1");
        let degraded = Health::Degraded {
            problems: vec!["player: exited with status 1".into()],
        };
        assert_eq!(status(&client).health, degraded);
        assert_eq!(read_snapshot(&path).health, degraded);

        // DEC-9 rule 3: control operations still work, and the repeated
        // dead-player error is recorded once, not per call.
        assert_eq!(
            client.request(parse_legacy("speed 1.5").unwrap()).unwrap(),
            Response::Ok
        );
        let after = status(&client);
        assert_eq!(after.speed, 1.5);
        assert_eq!(after.health, degraded);
        owner.shutdown();
    }

    #[test]
    fn dec9_player_follows_state_and_is_shut_down() {
        let dir = tempfile::tempdir().unwrap();
        let (owner, player, _path) = spawn(dir.path());
        let client = owner.client();
        client.request(parse_legacy("say hello").unwrap()).unwrap();
        client.request(parse_legacy("toggle").unwrap()).unwrap();
        client.request(parse_legacy("faster").unwrap()).unwrap();
        client.request(parse_legacy("stop").unwrap()).unwrap();
        owner.shutdown();
        assert_eq!(
            player.calls(),
            vec![
                PlayerCall::Stop,
                PlayerCall::SetPaused(true),
                PlayerCall::SetSpeed(1.0_f32 + 0.15),
                PlayerCall::Stop,
                PlayerCall::SetPaused(false),
                PlayerCall::Shutdown,
            ]
        );
    }

    #[test]
    fn quit_answers_bye_and_leaves_teardown_to_the_transport() {
        let dir = tempfile::tempdir().unwrap();
        let (owner, _player, _path) = spawn(dir.path());
        let client = owner.client();
        assert_eq!(
            client.request(Operation::Quit).unwrap(),
            Response::String { data: "bye".into() }
        );
        // Still serving: quitting is the transport's job.
        status(&client);
        owner.shutdown();
        let err = client.request(Operation::Status).unwrap_err();
        assert_eq!(err.code, vroca_core::operations::ErrorCode::Unavailable);
    }

    #[test]
    fn dec6_no_socket_is_bound_without_transport() {
        // DEC-6 / DEC-18: with default features the daemon core creates the
        // snapshot and nothing else in the runtime directory.
        let dir = tempfile::tempdir().unwrap();
        let (owner, _player, _path) = spawn(dir.path());
        status(&owner.client());
        owner.shutdown();
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["vroca-state.json".to_string()]);
    }
}
