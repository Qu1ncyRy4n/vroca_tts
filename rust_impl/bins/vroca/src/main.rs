#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::process::ExitCode;
use vroca_core::operations::{Operation, ProtocolError, Replace, ResetGroup, Scope, Unit};
use vroca_core::schema::{Aligner, EngineId, Position, VoiceRef};

#[cfg(feature = "transport")]
mod transport;

#[derive(Parser)]
#[command(name = "vroca", about = "Vroca TTS command line interface")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Read primary selection
    Read,
    /// Speak text, replacing active speech
    Say { text: Vec<String> },
    /// Speak text, replacing active speech (alias of say)
    Speak { text: Vec<String> },
    /// Queue text to be spoken
    Queue { text: Vec<String> },
    /// Stop active speech
    Stop,
    /// Clear queue and stop speech
    Clear,
    /// Skip current item
    Skip,
    /// Toggle pause/resume
    Toggle,
    /// Next sentence
    Next,
    /// Previous sentence
    Back,
    /// Faster speech
    Faster,
    /// Slower speech
    Slower,
    /// Set absolute speed
    Speed { value: f32 },
    /// Set voice by index, catalogue name, or engine:name
    Voice { voice: String },
    /// Set engine
    Engine { name: String },
    /// Set aligner
    Aligner { name: String },
    /// Set font size
    #[command(name = "font_size")]
    FontSize { size: i32 },
    /// Set words visible
    #[command(name = "words_visible")]
    WordsVisible { count: i32 },
    /// Set position (top, center, bottom)
    Position { pos: String },
    /// Cycle mode
    Mode,
    /// Reset preferences
    Reset,
    /// Reset preferences (alias)
    #[command(name = "reset_prefs")]
    ResetPrefs,
    /// Print status
    Status,
    /// Print catalogue
    Catalogue,
    /// Preview a voice by index, catalogue name, or engine:name
    Preview { voice: String },
    /// Unload engine
    Unload,
    /// Reload engine
    Reload,
    /// Quit daemon
    Quit,
    /// View daemon logs
    Log,
}

fn map_command(cmd: Commands) -> Result<Option<Operation>, ProtocolError> {
    match cmd {
        Commands::Read => Ok(Some(Operation::Read)),
        Commands::Say { text } | Commands::Speak { text } => {
            if text.is_empty() {
                return Err(ProtocolError::invalid_request("text required"));
            }
            Ok(Some(Operation::speak(text.join(" "), Replace::All, None)))
        }
        Commands::Queue { text } => {
            if text.is_empty() {
                return Err(ProtocolError::invalid_request("text required"));
            }
            Ok(Some(Operation::speak(text.join(" "), Replace::None, None)))
        }
        Commands::Stop => Ok(Some(Operation::stop(Scope::Playback))),
        Commands::Clear => Ok(Some(Operation::stop(Scope::All))),
        Commands::Skip => Ok(Some(Operation::skip(Unit::Item))),
        Commands::Toggle => Ok(Some(Operation::Toggle)),
        Commands::Next => Ok(Some(Operation::Next)),
        Commands::Back => Ok(Some(Operation::Back)),
        Commands::Faster => Ok(Some(Operation::Faster)),
        Commands::Slower => Ok(Some(Operation::Slower)),
        Commands::Speed { value } => Ok(Some(Operation::SetSpeed { speed: value })),
        Commands::Voice { voice } => Ok(Some(Operation::SetVoice {
            voice: VoiceRef::from_token(&voice),
        })),
        Commands::Engine { name } => {
            let id = EngineId::new(&name).map_err(ProtocolError::invalid_argument)?;
            Ok(Some(Operation::SetEngine { id }))
        }
        Commands::Aligner { name } => match Aligner::from_name(&name) {
            Some(aligner) => Ok(Some(Operation::SetAligner { aligner })),
            None => Err(ProtocolError::invalid_argument(format!(
                "unknown aligner: {name} (have {})",
                Aligner::NAMES.join(", ")
            ))),
        },
        Commands::FontSize { size } => Ok(Some(Operation::SetFontSize { size })),
        Commands::WordsVisible { count } => Ok(Some(Operation::SetWordsVisible { count })),
        Commands::Position { pos } => {
            let position = match pos.as_str() {
                "top" => Position::Top,
                "center" => Position::Center,
                "bottom" => Position::Bottom,
                _ => return Err(ProtocolError::invalid_argument("unknown position")),
            };
            Ok(Some(Operation::SetPosition { pos: position }))
        }
        Commands::Mode => Ok(Some(Operation::CycleMode)),
        Commands::Reset | Commands::ResetPrefs => Ok(Some(Operation::Reset {
            group: ResetGroup::Everything,
        })),
        Commands::Status => Ok(Some(Operation::Status)),
        Commands::Catalogue => Ok(Some(Operation::Catalogue)),
        Commands::Preview { voice } => Ok(Some(Operation::Preview {
            voice: VoiceRef::from_token(&voice),
        })),
        Commands::Unload => Ok(Some(Operation::Unload)),
        Commands::Reload => Ok(Some(Operation::Reload)),
        Commands::Quit => Ok(Some(Operation::Quit)),
        Commands::Log => Ok(None),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let cmd = cli.command.unwrap_or(Commands::Read);

    if matches!(cmd, Commands::Log) {
        return run_log();
    }

    let op = match map_command(cmd) {
        Ok(Some(op)) => op,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e.message);
            return ExitCode::from(e.exit_code());
        }
    };

    run_op(op)
}

#[cfg(feature = "transport")]
fn run_log() -> ExitCode {
    transport::log()
}

#[cfg(feature = "transport")]
fn run_op(op: Operation) -> ExitCode {
    transport::send(op)
}

#[cfg(not(feature = "transport"))]
fn run_log() -> ExitCode {
    without_transport()
}

#[cfg(not(feature = "transport"))]
fn run_op(_op: Operation) -> ExitCode {
    without_transport()
}

#[cfg(not(feature = "transport"))]
fn without_transport() -> ExitCode {
    eprintln!("error: vroca was built without the `transport` feature (rust-spec.md DEC-18)");
    ExitCode::FAILURE
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing_say_forwards_arguments() {
        let cli = Cli::try_parse_from(["vroca", "say", "hello", "world"]).unwrap();
        let cmd = cli.command.unwrap();
        let op = map_command(cmd).unwrap().unwrap();
        assert_eq!(op, Operation::speak("hello world", Replace::All, None));
    }

    #[test]
    fn test_cli_bare_means_read() {
        let cli = Cli::try_parse_from(["vroca"]).unwrap();
        let cmd = cli.command.unwrap_or(Commands::Read);
        let op = map_command(cmd).unwrap().unwrap();
        assert_eq!(op, Operation::Read);
    }

    #[test]
    fn test_cli_speed_arg() {
        let cli = Cli::try_parse_from(["vroca", "speed", "1.2"]).unwrap();
        let cmd = cli.command.unwrap();
        let op = map_command(cmd).unwrap().unwrap();
        assert_eq!(op, Operation::SetSpeed { speed: 1.2 });
    }

    #[test]
    fn test_cli_log_shortcut() {
        let cli = Cli::try_parse_from(["vroca", "log"]).unwrap();
        let cmd = cli.command.unwrap();
        let op = map_command(cmd).unwrap();
        assert!(op.is_none()); // meaning we skip daemon sending
    }

    #[test]
    fn test_cli_say_no_args_error() {
        let cli = Cli::try_parse_from(["vroca", "say"]).unwrap();
        let cmd = cli.command.unwrap();
        let err = map_command(cmd).unwrap_err();
        assert_eq!(err.code, vroca_core::operations::ErrorCode::InvalidRequest);
    }
}
