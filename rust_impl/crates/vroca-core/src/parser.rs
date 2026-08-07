use crate::operations::{Operation, ProtocolError, Replace, ResetGroup, Scope, Unit};
use crate::schema::{EngineId, Position};

pub fn parse_legacy(input: &str) -> Result<Operation, ProtocolError> {
    // Trim leading whitespace but NOT trailing or interior, per N7, though for the command
    // we need to split off the first word.
    let input = input.trim_start();
    let (cmd, rest) = if let Some(idx) = input.find(char::is_whitespace) {
        let (c, r) = input.split_at(idx);
        // Trim leading spaces from the rest, but preserve other interior/trailing spacing
        (
            c,
            r.trim_start_matches(|c: char| c.is_whitespace() && c != '\n'),
        )
    } else {
        (input, "")
    };

    match cmd {
        "read" => {
            if !rest.is_empty() {
                return Err(ProtocolError::invalid_request("read takes no arguments"));
            }
            Ok(Operation::Read)
        }
        "say" | "speak" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_request("text required"));
            }
            Ok(Operation::Speak {
                text: rest.to_string(),
                replace: Replace::All,
            })
        }
        "queue" => {
            if rest.is_empty() {
                return Err(ProtocolError::invalid_request("text required"));
            }
            Ok(Operation::Speak {
                text: rest.to_string(),
                replace: Replace::None,
            })
        }
        "stop" => Ok(Operation::Stop {
            scope: Scope::Playback,
        }),
        "clear" => Ok(Operation::Stop { scope: Scope::All }),
        "skip" => Ok(Operation::Skip { unit: Unit::Item }),
        "toggle" => Ok(Operation::Toggle), // Wait, Operation needs Toggle
        "next" => Ok(Operation::Next),
        "back" => Ok(Operation::Back),
        "faster" => Ok(Operation::Faster),
        "slower" => Ok(Operation::Slower),
        "speed" => {
            let val = rest
                .parse::<f32>()
                .map_err(|_| ProtocolError::invalid_argument("speed must be a float"))?;
            Ok(Operation::SetSpeed { speed: val })
        }
        "voice" => {
            let val = rest
                .parse::<i32>()
                .map_err(|_| ProtocolError::invalid_argument("voice must be an integer"))?;
            Ok(Operation::SetVoiceNumeric { sid: val })
        }
        "engine" => {
            let id = EngineId::new(rest).map_err(|e| ProtocolError::invalid_argument(e))?;
            Ok(Operation::SetEngine { id })
        }
        "aligner" => Ok(Operation::SetAligner {
            name: rest.to_string(),
        }),
        "font_size" => {
            let val = rest
                .parse::<i32>()
                .map_err(|_| ProtocolError::invalid_argument("font_size must be an integer"))?;
            Ok(Operation::SetFontSize { size: val })
        }
        "words_visible" => {
            let val = rest
                .parse::<i32>()
                .map_err(|_| ProtocolError::invalid_argument("words_visible must be an integer"))?;
            Ok(Operation::SetWordsVisible { count: val })
        }
        "position" => {
            let pos = match rest {
                "bottom" => Position::Bottom,
                "top" => Position::Top,
                "center" => Position::Center,
                _ => return Err(ProtocolError::invalid_argument("unknown position")),
            };
            Ok(Operation::SetPosition { pos })
        }
        "mode" => Ok(Operation::CycleMode),
        "reset" | "reset_prefs" => Ok(Operation::Reset {
            group: ResetGroup::Everything,
        }),
        "status" => Ok(Operation::Status),
        "catalogue" => Ok(Operation::Catalogue),
        "preview" => {
            let val = rest
                .parse::<i32>()
                .map_err(|_| ProtocolError::invalid_argument("preview must be an integer"))?;
            Ok(Operation::Preview { sid: val })
        }
        "unload" => Ok(Operation::Unload),
        "reload" => Ok(Operation::Reload),
        "quit" => Ok(Operation::Quit),
        _ => Err(ProtocolError::invalid_request(format!("unknown: {}", cmd))),
    }
}
