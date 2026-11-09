//! The legacy plaintext parser.
//!
//! One request is one command. The daemon strips the request's outer
//! whitespace and splits the verb off at the first whitespace run; the rest
//! is the argument with its interior spacing intact (N7). Every malformed
//! input returns a typed error (D8), and the error `message` is the exact
//! reply text `python_impl/daemon.py` sends, so a legacy client sees the
//! same bytes from either daemon.

use crate::operations::{Operation, ProtocolError, Replace, ResetGroup, Scope, Unit};
use crate::schema::{Aligner, EngineId, OverlayMode, Position, VoiceRef};

pub const SPEED_MIN: f32 = 0.5;
pub const SPEED_MAX: f32 = 3.0;
pub const VOLUME_MIN: i32 = 0;
pub const VOLUME_MAX: i32 = 100;
pub const TRIM_MIN: i32 = -100;
pub const TRIM_MAX: i32 = 100;
pub const FONT_SIZE_MIN: i32 = 12;
pub const FONT_SIZE_MAX: i32 = 72;
pub const WORDS_VISIBLE_MIN: i32 = 1;
pub const WORDS_VISIBLE_MAX: i32 = 15;

pub fn parse_legacy(input: &str) -> Result<Operation, ProtocolError> {
    let command = input.trim();
    let (verb, rest) = match command.find(char::is_whitespace) {
        Some(idx) => {
            let (verb, rest) = command.split_at(idx);
            (verb, rest.trim_start())
        }
        None => (command, ""),
    };

    // Python's `unknown: <cmd>` echoes the full stripped request, and a
    // no-argument verb with trailing text falls through to it because the
    // whole-string comparison fails (DEC-13, review bug 2).
    let unknown = || ProtocolError::invalid_request(format!("unknown: {command}"));
    let bare = |op: Operation| {
        if rest.is_empty() {
            Ok(op)
        } else {
            Err(unknown())
        }
    };

    match verb {
        "read" => bare(Operation::Read),
        "stop" => bare(Operation::Stop {
            scope: Scope::Playback,
        }),
        "clear" => bare(Operation::Stop { scope: Scope::All }),
        "skip" => bare(Operation::Skip { unit: Unit::Item }),
        "toggle" => bare(Operation::Toggle),
        "next" => bare(Operation::Next),
        "back" => bare(Operation::Back),
        "faster" => bare(Operation::Faster),
        "slower" => bare(Operation::Slower),
        "mode" => bare(Operation::CycleMode),
        "status" => bare(Operation::Status),
        "catalogue" => bare(Operation::Catalogue),
        "unload" => bare(Operation::Unload),
        "reload" => bare(Operation::Reload),
        "quit" => bare(Operation::Quit),

        "say" | "speak" | "queue" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_request(format!("{verb} needs text")));
            }
            let (voice, text) = split_voice(rest);
            if text.is_empty() {
                return Err(ProtocolError::invalid_request(format!("{verb} needs text")));
            }
            let replace = if verb == "queue" {
                Replace::None
            } else {
                Replace::All
            };
            Ok(Operation::Speak {
                text: text.to_string(),
                replace,
                voice,
            })
        }

        "speed" => {
            let speed = rest.parse::<f32>().ok().filter(|v| v.is_finite());
            match speed {
                Some(speed) => Ok(Operation::SetSpeed { speed }),
                None => Err(needs_number("speed", SPEED_MIN, SPEED_MAX)),
            }
        }
        "volume" => match rest.parse::<i32>() {
            Ok(level) => Ok(Operation::SetVolume { level }),
            Err(_) => Err(needs_number("volume", VOLUME_MIN, VOLUME_MAX)),
        },
        "trim" => match rest.parse::<i32>() {
            Ok(amount) => Ok(Operation::SetTrim { amount }),
            Err(_) => Err(needs_number("trim", TRIM_MIN, TRIM_MAX)),
        },
        "font_size" => match rest.parse::<i32>() {
            Ok(size) => Ok(Operation::SetFontSize { size }),
            Err(_) => Err(needs_number("font_size", FONT_SIZE_MIN, FONT_SIZE_MAX)),
        },
        "words_visible" => match rest.parse::<i32>() {
            Ok(count) => Ok(Operation::SetWordsVisible { count }),
            Err(_) => Err(needs_number(
                "words_visible",
                WORDS_VISIBLE_MIN,
                WORDS_VISIBLE_MAX,
            )),
        },

        "voice" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_argument(
                    "voice needs an index or name",
                ));
            }
            Ok(Operation::SetVoice {
                voice: VoiceRef::from_token(rest),
            })
        }
        "preview" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_argument("preview needs a voice"));
            }
            Ok(Operation::Preview {
                voice: VoiceRef::from_token(rest),
            })
        }
        "engine" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_argument("engine needs a name"));
            }
            let id = EngineId::new(rest).map_err(ProtocolError::invalid_argument)?;
            Ok(Operation::SetEngine { id })
        }
        "aligner" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_argument("aligner needs a name"));
            }
            match Aligner::from_name(rest) {
                Some(aligner) => Ok(Operation::SetAligner { aligner }),
                None => Err(ProtocolError::invalid_argument(format!(
                    "unknown aligner: {rest} (have {})",
                    Aligner::NAMES.join(", ")
                ))),
            }
        }
        "position" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_argument("position needs a name"));
            }
            match Position::from_name(rest) {
                Some(pos) => Ok(Operation::SetPosition { pos }),
                None => Err(ProtocolError::invalid_argument(format!(
                    "unknown position: {rest}"
                ))),
            }
        }
        "mode_set" => match OverlayMode::from_name(rest) {
            Some(mode) => Ok(Operation::SetMode { mode }),
            None => Err(ProtocolError::invalid_argument(format!(
                "unknown mode: {rest} (have {})",
                OverlayMode::NAMES.join(", ")
            ))),
        },
        "reset" | "reset_prefs" => {
            if rest.is_empty() {
                return Ok(Operation::Reset {
                    group: ResetGroup::Everything,
                });
            }
            // DEC-15: an unknown group is `invalid_argument`. Python has no
            // groups and would reply `unknown: reset <x>`; this is a recorded
            // deviation in favour of the decision.
            match ResetGroup::from_name(rest) {
                Some(group) => Ok(Operation::Reset { group }),
                None => Err(ProtocolError::invalid_argument(format!(
                    "unknown reset group: {rest} (have {})",
                    ResetGroup::NAMES.join(", ")
                ))),
            }
        }

        _ => Err(unknown()),
    }
}

/// Pull a leading `--voice TOKEN` off the text, as `daemon.py` `_split_voice`
/// does. The token is one word; the remainder keeps its interior spacing.
fn split_voice(text: &str) -> (Option<VoiceRef>, &str) {
    let Some(after) = text.strip_prefix("--voice ") else {
        return (None, text);
    };
    let after = after.trim_start();
    match after.find(char::is_whitespace) {
        Some(idx) => {
            let (token, body) = after.split_at(idx);
            (Some(VoiceRef::from_token(token)), body.trim_start())
        }
        None => {
            if after.is_empty() {
                (None, "")
            } else {
                (Some(VoiceRef::from_token(after)), "")
            }
        }
    }
}

fn needs_number<T: std::fmt::Display>(field: &str, min: T, max: T) -> ProtocolError {
    ProtocolError::invalid_argument(format!("{field} needs a number between {min} and {max}"))
}
