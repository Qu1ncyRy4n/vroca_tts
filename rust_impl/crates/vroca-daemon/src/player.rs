//! The player boundary (DEC-9).
//!
//! The state owner drives playback only through `Player`, so mpv (slice 6),
//! libmpv, or a native backend can replace the fake without touching domain
//! code. Asynchronous events — above all, the player dying (N4) — come back
//! through the `EventSink` the owner attaches before any other call.

use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerError {
    /// The player process or backend is gone. Carries the reason.
    Dead(String),
}

impl PlayerError {
    /// The health problem this error reports. Identical for repeated calls
    /// against a dead player, so health records it once.
    pub fn problem(&self) -> String {
        match self {
            Self::Dead(reason) => format!("player: {reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    Died { reason: String },
}

pub type EventSink = Box<dyn Fn(PlayerEvent) + Send>;

pub trait Player: Send {
    /// Called once by the state owner before any other method.
    fn attach(&mut self, events: EventSink);
    fn set_paused(&mut self, paused: bool) -> Result<(), PlayerError>;
    fn set_speed(&mut self, speed: f32) -> Result<(), PlayerError>;
    /// Stop playback and drop anything loaded (rendered audio is stale).
    fn stop(&mut self) -> Result<(), PlayerError>;
    /// Release the player for good: terminate a child, close IPC. Called
    /// exactly once, on shutdown (DEC-9 rule 2).
    fn shutdown(&mut self);
}

/// What a `FakePlayer` was asked to do, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCall {
    SetPaused(bool),
    SetSpeed(f32),
    Stop,
    Shutdown,
}

#[derive(Default)]
struct FakeShared {
    dead: Option<String>,
    calls: Vec<PlayerCall>,
    events: Option<EventSink>,
}

/// In-process fake: records calls, and can be killed from a test through its
/// `FakePlayerHandle` to exercise the N4 path. Spawns nothing.
pub struct FakePlayer {
    shared: Arc<Mutex<FakeShared>>,
}

/// Test-side view of a `FakePlayer` after it has been handed to the owner.
#[derive(Clone)]
pub struct FakePlayerHandle {
    shared: Arc<Mutex<FakeShared>>,
}

fn lock(shared: &Mutex<FakeShared>) -> MutexGuard<'_, FakeShared> {
    // A panic while holding this lock cannot leave the call log invalid.
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl FakePlayer {
    pub fn new() -> (Self, FakePlayerHandle) {
        let shared = Arc::new(Mutex::new(FakeShared::default()));
        (
            Self {
                shared: shared.clone(),
            },
            FakePlayerHandle { shared },
        )
    }

    fn record(&self, call: PlayerCall) -> Result<(), PlayerError> {
        let mut shared = lock(&self.shared);
        if let Some(reason) = &shared.dead {
            return Err(PlayerError::Dead(reason.clone()));
        }
        shared.calls.push(call);
        Ok(())
    }
}

impl Player for FakePlayer {
    fn attach(&mut self, events: EventSink) {
        lock(&self.shared).events = Some(events);
    }

    fn set_paused(&mut self, paused: bool) -> Result<(), PlayerError> {
        self.record(PlayerCall::SetPaused(paused))
    }

    fn set_speed(&mut self, speed: f32) -> Result<(), PlayerError> {
        self.record(PlayerCall::SetSpeed(speed))
    }

    fn stop(&mut self) -> Result<(), PlayerError> {
        self.record(PlayerCall::Stop)
    }

    fn shutdown(&mut self) {
        lock(&self.shared).calls.push(PlayerCall::Shutdown);
    }
}

impl FakePlayerHandle {
    /// Simulate the player dying underneath the daemon, as mpv can (N4).
    pub fn kill(&self, reason: &str) {
        let events = {
            let mut shared = lock(&self.shared);
            shared.dead = Some(reason.to_string());
            shared.events.take()
        };
        if let Some(events) = events {
            events(PlayerEvent::Died {
                reason: reason.to_string(),
            });
        }
    }

    pub fn calls(&self) -> Vec<PlayerCall> {
        lock(&self.shared).calls.clone()
    }
}
