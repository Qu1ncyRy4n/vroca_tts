//! The state machine. `State::apply` is the single place an `Operation`
//! changes state, and every call answers with a typed `Reply` or a typed
//! error: nothing claims success it did not have (review bug 5). Reply texts
//! are daemon.py's, so the legacy socket keeps its bytes.
//!
//! Queues are keyed by channel (§10.9) with one default channel. Playback
//! pause is global, since there is one player. Rendered audio is
//! invalidated by bumping `generation` (DEC-4), which the daemon turns into
//! a player stop.

use std::collections::{BTreeMap, VecDeque};

use crate::catalogue::{Catalogue, ResolvedVoice};
use crate::operations::{ErrorCode, Operation, ProtocolError, Replace, ResetGroup, Scope, Unit};
use crate::parser::{
    FONT_SIZE_MAX, FONT_SIZE_MIN, SPEED_MAX, SPEED_MIN, TRIM_MAX, TRIM_MIN, VOLUME_MAX, VOLUME_MIN,
    WORDS_VISIBLE_MAX, WORDS_VISIBLE_MIN,
};
use crate::schema::{ChannelId, EngineId, Preferences, VoiceRef};
use crate::text::sentences;

/// `faster` / `slower` step, as daemon.py `SPEED_STEP`.
pub const SPEED_STEP: f32 = 0.15;

/// Submitted text plus its derived sentences (DEC-16 `SpeechItem`).
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechItem {
    pub text: String,
    pub sentences: Vec<String>,
    /// Index of the sentence playing now; always `< sentences.len()`.
    pub index: usize,
    /// The voice the item was submitted with, if not the current one. It
    /// travels with the item, as in daemon.py.
    pub voice: Option<ResolvedVoice>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChannelQueue {
    pub active: Option<SpeechItem>,
    pub waiting: VecDeque<SpeechItem>,
}

/// Where reading resumes after a preview (DEC-14).
#[derive(Debug, Clone, PartialEq)]
struct RestorePoint {
    index: usize,
    paused: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    Text(String),
    /// The daemon answers with its snapshot.
    Status,
    Catalogue(Catalogue),
    /// Synthesize and play this voice's preview, then call
    /// `State::end_preview` whether it worked or not.
    Preview(ResolvedVoice),
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub prefs: Preferences,
    pub generation: u64,
    pub paused: bool,
    channels: BTreeMap<ChannelId, ChannelQueue>,
    catalogues: BTreeMap<EngineId, Catalogue>,
    preview: Option<RestorePoint>,
}

fn text(s: impl Into<String>) -> Result<Reply, ProtocolError> {
    Ok(Reply::Text(s.into()))
}

fn unsupported(message: impl Into<String>) -> ProtocolError {
    ProtocolError {
        code: ErrorCode::UnsupportedOperation,
        message: message.into(),
    }
}

impl State {
    pub fn new(prefs: Preferences) -> Self {
        let mut channels = BTreeMap::new();
        channels.insert(ChannelId::default(), ChannelQueue::default());
        Self {
            prefs,
            generation: 0,
            paused: false,
            channels,
            catalogues: BTreeMap::new(),
            preview: None,
        }
    }

    /// Install one engine's voice list. Catalogues are static data (DEC-11).
    pub fn set_catalogue(&mut self, catalogue: Catalogue) {
        self.catalogues.insert(catalogue.engine.clone(), catalogue);
    }

    pub fn channel(&self, id: &ChannelId) -> Option<&ChannelQueue> {
        self.channels.get(id)
    }

    /// The default channel, the only one until §10.8 10-b is decided.
    pub fn default_channel(&self) -> Option<&ChannelQueue> {
        self.channel(&ChannelId::default())
    }

    pub fn active_item(&self) -> Option<&SpeechItem> {
        self.default_channel().and_then(|q| q.active.as_ref())
    }

    pub fn waiting_len(&self) -> usize {
        self.default_channel().map_or(0, |q| q.waiting.len())
    }

    pub fn is_active(&self) -> bool {
        self.channels.values().any(|q| q.active.is_some())
    }

    pub fn previewing(&self) -> bool {
        self.preview.is_some()
    }

    /// Effective player level: master volume plus the current voice's trim.
    pub fn effective_volume(&self) -> i32 {
        let trim = self
            .prefs
            .trims
            .get(&self.voice_key())
            .copied()
            .unwrap_or(0);
        self.prefs
            .volume
            .saturating_add(trim)
            .clamp(VOLUME_MIN, VOLUME_MAX)
    }

    fn voice_key(&self) -> String {
        format!("{}:{}", self.prefs.engine, self.prefs.voice)
    }

    fn queue_mut(&mut self, id: &ChannelId) -> &mut ChannelQueue {
        self.channels.entry(id.clone()).or_default()
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// A new active item starts playing from its first sentence.
    fn started(&mut self) {
        self.paused = false;
        self.preview = None;
        self.bump_generation();
    }

    fn stop_playback(&mut self, id: &ChannelId) {
        self.queue_mut(id).active = None;
        if !self.is_active() {
            self.paused = false;
        }
        self.preview = None;
        self.bump_generation();
    }

    pub fn apply(&mut self, op: Operation) -> Result<Reply, ProtocolError> {
        match op {
            Operation::Speak {
                channel,
                text: submitted,
                replace,
                voice,
            } => self.speak(&channel, submitted, replace, voice.as_ref()),
            Operation::Stop { channel, scope } => match scope {
                Scope::Playback => {
                    self.stop_playback(&channel);
                    text("stopped")
                }
                Scope::Queue => {
                    // Prefetched audio for waiting items is stale too.
                    self.queue_mut(&channel).waiting.clear();
                    self.bump_generation();
                    text("queue cleared")
                }
                Scope::All => {
                    self.queue_mut(&channel).waiting.clear();
                    self.stop_playback(&channel);
                    text("queue cleared")
                }
            },
            Operation::Skip { channel, unit } => match unit {
                Unit::Item => self.skip_item(&channel),
                Unit::Sentence => self.seek(1),
            },
            Operation::Toggle => {
                if !self.is_active() {
                    return text("idle");
                }
                self.paused = !self.paused;
                text(if self.paused { "paused" } else { "resumed" })
            }
            Operation::Pause | Operation::Resume => {
                if !self.is_active() {
                    return text("idle");
                }
                self.paused = op == Operation::Pause;
                text(if self.paused { "paused" } else { "resumed" })
            }
            Operation::Read => {
                // DEC-13: `read` stops when active, reads the selection when
                // idle. Capturing the selection is daemon I/O not yet built.
                if self.is_active() {
                    self.stop_playback(&ChannelId::default());
                    return text("stopped");
                }
                Err(unsupported("reading the selection is not implemented yet"))
            }
            Operation::ReadSelection => {
                Err(unsupported("reading the selection is not implemented yet"))
            }
            Operation::Next => self.seek(1),
            Operation::Back => self.seek(-1),
            Operation::Faster => self.set_speed(self.prefs.speed + SPEED_STEP),
            Operation::Slower => self.set_speed(self.prefs.speed - SPEED_STEP),
            Operation::SetSpeed { speed } => self.set_speed(speed),
            Operation::SetVoice { voice } => self.set_voice(&voice),
            Operation::SetEngine { id } => {
                if id != self.prefs.engine {
                    // N10: an engine change invalidates rendered audio. The
                    // voice index is engine-local, so it resets (daemon.py).
                    self.prefs.engine = id;
                    self.prefs.voice = 0;
                    self.prefs.voice_id = None;
                    self.bump_generation();
                }
                text(format!("engine {}", self.prefs.engine))
            }
            Operation::SetAligner { aligner } => {
                self.prefs.aligner = aligner;
                text(format!("aligner {}", aligner.as_str()))
            }
            Operation::SetFontSize { size } => {
                self.prefs.font_size = size.clamp(FONT_SIZE_MIN, FONT_SIZE_MAX);
                text(format!("font_size {}", self.prefs.font_size))
            }
            Operation::SetWordsVisible { count } => {
                self.prefs.words_visible = count.clamp(WORDS_VISIBLE_MIN, WORDS_VISIBLE_MAX);
                text(format!("words_visible {}", self.prefs.words_visible))
            }
            Operation::SetPosition { pos } => {
                self.prefs.position = pos;
                text(format!("position {}", pos.as_str()))
            }
            Operation::SetVolume { level } => {
                self.prefs.volume = level.clamp(VOLUME_MIN, VOLUME_MAX);
                text(format!("volume {}", self.prefs.volume))
            }
            Operation::SetTrim { amount } => {
                let key = self.voice_key();
                let amount = amount.clamp(TRIM_MIN, TRIM_MAX);
                if amount == 0 {
                    self.prefs.trims.remove(&key);
                } else {
                    self.prefs.trims.insert(key.clone(), amount);
                }
                text(format!(
                    "trim {key} {amount:+} (effective {})",
                    self.effective_volume()
                ))
            }
            Operation::CycleMode => {
                self.prefs.overlay_mode = self.prefs.overlay_mode.next();
                text(self.prefs.overlay_mode.as_str())
            }
            Operation::SetMode { mode } => {
                self.prefs.overlay_mode = mode;
                text(mode.as_str())
            }
            Operation::Reset { group } => Ok(self.reset(group)),
            Operation::Status => Ok(Reply::Status),
            Operation::Catalogue => match self.catalogues.get(&self.prefs.engine) {
                Some(catalogue) => Ok(Reply::Catalogue(catalogue.clone())),
                None => Err(unsupported(format!(
                    "no voice catalogue for {} yet",
                    self.prefs.engine
                ))),
            },
            Operation::Preview { voice } => {
                // DEC-14: an unresolvable voice is not_found and touches
                // nothing. Otherwise save the restore point once: a second
                // preview replaces the first without losing it.
                let resolved = self.resolve(&voice)?.0;
                if self.preview.is_none() {
                    let index = self.active_item().map_or(0, |item| item.index);
                    self.preview = Some(RestorePoint {
                        index,
                        paused: self.paused,
                    });
                }
                if self.is_active() {
                    self.paused = true;
                }
                Ok(Reply::Preview(resolved))
            }
            Operation::Unload => text(format!("unloaded {}", self.prefs.engine)),
            Operation::Reload => Err(ProtocolError {
                code: ErrorCode::EngineFailure,
                message: "reload failed: no engine is implemented yet".into(),
            }),
            Operation::Quit => text("bye"),
        }
    }

    /// Restore reading after a preview, whether it played or failed
    /// (DEC-14: the restore point is never lost on an error path).
    pub fn end_preview(&mut self) {
        let Some(point) = self.preview.take() else {
            return;
        };
        let active = self.is_active();
        if let Some(item) = self
            .channels
            .get_mut(&ChannelId::default())
            .and_then(|q| q.active.as_mut())
        {
            item.index = point.index.min(item.sentences.len().saturating_sub(1));
        }
        self.paused = active && point.paused;
        // The preview replaced the loaded sentence; reload it.
        self.bump_generation();
    }

    fn speak(
        &mut self,
        channel: &ChannelId,
        submitted: String,
        replace: Replace,
        voice: Option<&VoiceRef>,
    ) -> Result<Reply, ProtocolError> {
        let voice = voice.map(|v| self.resolve(v).map(|r| r.0)).transpose()?;
        let parts = sentences(&submitted);
        if parts.is_empty() {
            // daemon.py's replies for text that normalizes to nothing; the
            // state is left untouched.
            let message = if replace == Replace::None {
                "empty text"
            } else {
                "empty"
            };
            return Err(ProtocolError::invalid_argument(message));
        }
        let n = parts.len();
        let item = SpeechItem {
            text: submitted,
            sentences: parts,
            index: 0,
            voice,
        };
        let queue = self.queue_mut(channel);
        match replace {
            Replace::All | Replace::Active => {
                if replace == Replace::All {
                    queue.waiting.clear();
                }
                queue.active = Some(item);
                self.started();
                text(format!("reading {n}"))
            }
            Replace::None => {
                if queue.active.is_none() {
                    queue.active = Some(item);
                    self.started();
                    text(format!("playing {n} sentences"))
                } else {
                    queue.waiting.push_back(item);
                    let len = queue.waiting.len();
                    text(format!("queued {n} sentences (queue length: {len})"))
                }
            }
        }
    }

    fn skip_item(&mut self, channel: &ChannelId) -> Result<Reply, ProtocolError> {
        let queue = self.queue_mut(channel);
        match queue.waiting.pop_front() {
            Some(next) => {
                queue.active = Some(next);
                let len = queue.waiting.len();
                self.started();
                text(format!("skipped item (queue length: {len})"))
            }
            None => {
                self.stop_playback(channel);
                text("stopped")
            }
        }
    }

    /// Move within the active item, clamped to its sentences (review bug 3).
    /// At either end the current sentence replays, as daemon.py `seek`.
    fn seek(&mut self, delta: isize) -> Result<Reply, ProtocolError> {
        let Some(item) = self
            .channels
            .get_mut(&ChannelId::default())
            .and_then(|q| q.active.as_mut())
        else {
            return text("idle");
        };
        let last = item.sentences.len().saturating_sub(1);
        item.index = item.index.saturating_add_signed(delta).min(last);
        let position = format!("{}/{}", item.index + 1, item.sentences.len());
        self.preview = None;
        self.bump_generation();
        text(position)
    }

    fn set_speed(&mut self, speed: f32) -> Result<Reply, ProtocolError> {
        // Two decimals, so repeated faster/slower lands on exact steps
        // instead of accumulating float error.
        self.prefs.speed = ((speed * 100.0).round() / 100.0).clamp(SPEED_MIN, SPEED_MAX);
        text(format!("speed {:.2}", self.prefs.speed))
    }

    fn set_voice(&mut self, voice: &VoiceRef) -> Result<Reply, ProtocolError> {
        let (resolved, engine) = self.resolve(voice)?;
        let switched = engine != self.prefs.engine;
        self.prefs.engine = engine;
        self.prefs.voice = resolved.speaker.0;
        self.prefs.voice_id = Some(resolved.id);
        // N9: a voice change invalidates audio rendered in the old voice.
        self.bump_generation();
        let reply = format!("voice {}", self.prefs.voice);
        if switched {
            text(format!("{reply} (engine {})", self.prefs.engine))
        } else {
            text(reply)
        }
    }

    /// Resolve against the named engine's catalogue for `engine:name`, else
    /// the current engine's (DEC-10).
    fn resolve(&self, voice: &VoiceRef) -> Result<(ResolvedVoice, EngineId), ProtocolError> {
        let engine = match voice {
            VoiceRef::Qualified(id) => {
                id.0.split_once(':')
                    .and_then(|(engine, _)| EngineId::new(engine).ok())
                    .unwrap_or_else(|| self.prefs.engine.clone())
            }
            _ => self.prefs.engine.clone(),
        };
        let Some(catalogue) = self.catalogues.get(&engine) else {
            let token = match voice {
                VoiceRef::Index(i) => i.to_string(),
                VoiceRef::Name(name) => name.clone(),
                VoiceRef::Qualified(id) => id.0.clone(),
            };
            return Err(ProtocolError::not_found(format!(
                "unknown voice: {token} (no voice catalogue for {engine} yet)"
            )));
        };
        Ok((catalogue.resolve(voice)?, engine))
    }

    fn reset(&mut self, group: ResetGroup) -> Reply {
        let defaults = Preferences::default();
        match group {
            ResetGroup::Everything => {
                // DEC-15: every preference including engine, playback stopped,
                // rendered audio invalidated. Fields from a newer writer are
                // not ours to reset.
                let unknown = std::mem::take(&mut self.prefs.unknown_fields);
                self.prefs = Preferences {
                    unknown_fields: unknown,
                    ..defaults
                };
                for queue in self.channels.values_mut() {
                    *queue = ChannelQueue::default();
                }
                self.paused = false;
                self.preview = None;
                self.bump_generation();
                Reply::Text("reset to defaults".into())
            }
            ResetGroup::Overlay => {
                self.prefs.overlay_mode = defaults.overlay_mode;
                self.prefs.position = defaults.position;
                self.prefs.font_size = defaults.font_size;
                self.prefs.words_visible = defaults.words_visible;
                Reply::Text("reset overlay to defaults".into())
            }
            ResetGroup::Speech => {
                self.prefs.engine = defaults.engine;
                self.prefs.voice = defaults.voice;
                self.prefs.voice_id = defaults.voice_id;
                self.prefs.speed = defaults.speed;
                self.prefs.aligner = defaults.aligner;
                self.bump_generation();
                Reply::Text("reset speech to defaults".into())
            }
        }
    }
}
