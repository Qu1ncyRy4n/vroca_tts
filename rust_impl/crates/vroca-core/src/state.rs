use crate::operations::{Operation, Replace, ResetGroup, Scope, Unit};
use crate::schema::{Aligner, EngineId, Position, Preferences, VoiceId};

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechItem {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub prefs: Preferences,
    pub generation: u64,
    pub active_item: Option<SpeechItem>,
    pub waiting_queue: Vec<SpeechItem>,
    pub paused: bool,
    pub active_sentence_index: usize,
    pub sentences: Vec<String>,
}

impl State {
    pub fn new(prefs: Preferences) -> Self {
        Self {
            prefs,
            generation: 0,
            active_item: None,
            waiting_queue: Vec::new(),
            paused: false,
            active_sentence_index: 0,
            sentences: Vec::new(),
        }
    }

    pub fn apply(&mut self, op: Operation) {
        match op {
            Operation::Speak {
                text,
                replace,
                voice: _,
            } => {
                let item = SpeechItem { text };
                match replace {
                    Replace::All => {
                        self.active_item = Some(item);
                        self.waiting_queue.clear();
                        self.paused = false;
                        self.active_sentence_index = 0;
                        self.bump_generation();
                    }
                    Replace::Active => {
                        self.active_item = Some(item);
                        self.paused = false;
                        self.active_sentence_index = 0;
                        self.bump_generation();
                    }
                    Replace::None => {
                        if self.active_item.is_none() {
                            self.active_item = Some(item);
                            self.paused = false;
                            self.active_sentence_index = 0;
                            self.bump_generation();
                        } else {
                            self.waiting_queue.push(item);
                        }
                    }
                }
            }
            Operation::Stop { scope } => match scope {
                Scope::Playback => {
                    self.active_item = None;
                    self.paused = false;
                    self.active_sentence_index = 0;
                    self.bump_generation();
                }
                Scope::Queue => {
                    self.waiting_queue.clear();
                }
                Scope::All => {
                    self.active_item = None;
                    self.waiting_queue.clear();
                    self.paused = false;
                    self.active_sentence_index = 0;
                    self.bump_generation();
                }
            },
            Operation::Skip { unit } => match unit {
                Unit::Item => {
                    self.active_item = None;
                    self.paused = false;
                    self.active_sentence_index = 0;
                    self.bump_generation();
                    if !self.waiting_queue.is_empty() {
                        self.active_item = Some(self.waiting_queue.remove(0));
                    }
                }
                Unit::Sentence => {
                    self.active_sentence_index += 1;
                    self.bump_generation();
                }
            },
            Operation::Pause => {
                self.paused = true;
            }
            Operation::Resume => {
                self.paused = false;
            }
            Operation::Toggle => {
                self.paused = !self.paused;
            }
            Operation::ReadSelection | Operation::Read => {
                if self.active_item.is_some() {
                    self.apply(Operation::Stop {
                        scope: Scope::Playback,
                    });
                } else {
                    // Handled externally for the actual clipboard read,
                    // but the state machine itself just treats it as stopping if active.
                }
            }
            Operation::Next => {
                self.active_sentence_index += 1;
                self.bump_generation();
            }
            Operation::Back => {
                if self.active_sentence_index > 0 {
                    self.active_sentence_index -= 1;
                }
                self.bump_generation();
            }
            Operation::Faster => {
                self.prefs.speed = (self.prefs.speed + 0.15).min(3.0);
            }
            Operation::Slower => {
                self.prefs.speed = (self.prefs.speed - 0.15).max(0.5);
            }
            Operation::SetSpeed { speed } => {
                self.prefs.speed = speed.clamp(0.5, 3.0);
            }
            Operation::SetVoice { voice: _ } => {
                // Catalogue resolution to (ModelId, SpeakerIndex) is slice B;
                // a voice change invalidates rendered audio regardless (N9).
                self.bump_generation();
            }
            Operation::SetEngine { id } => {
                self.prefs.engine = id;
                self.bump_generation();
            }
            Operation::SetAligner { aligner } => {
                self.prefs.aligner = aligner;
            }
            Operation::SetFontSize { size } => {
                self.prefs.font_size = size.clamp(12, 72);
            }
            Operation::SetWordsVisible { count } => {
                self.prefs.words_visible = count.clamp(1, 15);
            }
            Operation::SetPosition { pos } => {
                self.prefs.position = pos;
            }
            Operation::CycleMode => {
                self.prefs.overlay_mode = self.prefs.overlay_mode.next();
            }
            Operation::SetMode { mode } => {
                self.prefs.overlay_mode = mode;
            }
            Operation::Reset { group } => match group {
                ResetGroup::Everything => {
                    self.prefs = Default::default();
                    self.apply(Operation::Stop { scope: Scope::All });
                    self.bump_generation();
                }
                ResetGroup::Overlay => {
                    self.prefs.font_size = 24;
                    self.prefs.words_visible = 3;
                    self.prefs.position = Position::Bottom;
                    self.prefs.overlay_mode = crate::schema::OverlayMode::Subtitle;
                }
                ResetGroup::Speech => {
                    self.prefs.engine = EngineId::default_engine();
                    self.prefs.voice = VoiceId("kokoro:af_bella".to_string());
                    self.prefs.speed = 1.0;
                    self.prefs.aligner = Aligner::Asr;
                    self.bump_generation();
                }
            },
            _ => {}
        }
    }

    fn bump_generation(&mut self) {
        self.generation += 1;
    }
}
