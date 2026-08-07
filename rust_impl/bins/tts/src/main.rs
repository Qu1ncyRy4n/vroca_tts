#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::process::Command;
use vroca_client::Client;
use vroca_core::operations::{Operation, ProtocolError, Replace, ResetGroup, Scope, Unit};
use vroca_core::schema::{EngineId, Position};

#[derive(Parser)]
#[command(name = "tts", about = "Vroca TTS command line interface")]
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
    /// Set voice by index
    Voice { sid: i32 },
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
    /// Preview a voice by index
    Preview { sid: i32 },
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
            Ok(Some(Operation::Speak {
                text: text.join(" "),
                replace: Replace::All,
            }))
        }
        Commands::Queue { text } => {
            if text.is_empty() {
                return Err(ProtocolError::invalid_request("text required"));
            }
            Ok(Some(Operation::Speak {
                text: text.join(" "),
                replace: Replace::None,
            }))
        }
        Commands::Stop => Ok(Some(Operation::Stop {
            scope: Scope::Playback,
        })),
        Commands::Clear => Ok(Some(Operation::Stop { scope: Scope::All })),
        Commands::Skip => Ok(Some(Operation::Skip { unit: Unit::Item })),
        Commands::Toggle => Ok(Some(Operation::Toggle)),
        Commands::Next => Ok(Some(Operation::Next)),
        Commands::Back => Ok(Some(Operation::Back)),
        Commands::Faster => Ok(Some(Operation::Faster)),
        Commands::Slower => Ok(Some(Operation::Slower)),
        Commands::Speed { value } => Ok(Some(Operation::SetSpeed { speed: value })),
        Commands::Voice { sid } => Ok(Some(Operation::SetVoiceNumeric { sid })),
        Commands::Engine { name } => {
            let id = EngineId::new(&name).map_err(ProtocolError::invalid_argument)?;
            Ok(Some(Operation::SetEngine { id }))
        }
        Commands::Aligner { name } => Ok(Some(Operation::SetAligner { name })),
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
        Commands::Preview { sid } => Ok(Some(Operation::Preview { sid })),
        Commands::Unload => Ok(Some(Operation::Unload)),
        Commands::Reload => Ok(Some(Operation::Reload)),
        Commands::Quit => Ok(Some(Operation::Quit)),
        Commands::Log => Ok(None),
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let cmd = cli.command.unwrap_or(Commands::Read);

    if matches!(cmd, Commands::Log) {
        let status = Command::new("journalctl")
            .args(["--user", "-u", "tts", "-u", "tts-overlay", "-f"])
            .status();

        match status {
            Ok(s) if s.success() => return std::process::ExitCode::SUCCESS,
            _ => return std::process::ExitCode::FAILURE,
        }
    }

    let op = match map_command(cmd) {
        Ok(Some(op)) => op,
        Ok(None) => return std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e.message);
            return std::process::ExitCode::FAILURE;
        }
    };

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket_path = std::path::PathBuf::from(runtime_dir).join("vroca-v1.sock");

    let client = Client::new(socket_path);
    match client.send(op) {
        Ok(res) => {
            match res {
                vroca_core::codec::Response::Ok => {}
                vroca_core::codec::Response::String { data } => println!("{}", data),
                vroca_core::codec::Response::Status { snapshot } => {
                    println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
                }
                vroca_core::codec::Response::Catalogue { data } => {
                    println!("{}", serde_json::to_string_pretty(&data).unwrap());
                }
                vroca_core::codec::Response::Error { error } => {
                    eprintln!("error: {}", error.message);
                    return std::process::ExitCode::FAILURE;
                }
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}", e.message);
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing_say_forwards_arguments() {
        let cli = Cli::try_parse_from(["tts", "say", "hello", "world"]).unwrap();
        let cmd = cli.command.unwrap();
        let op = map_command(cmd).unwrap().unwrap();
        assert_eq!(
            op,
            Operation::Speak {
                text: "hello world".to_string(),
                replace: Replace::All
            }
        );
    }

    #[test]
    fn test_cli_bare_means_read() {
        let cli = Cli::try_parse_from(["tts"]).unwrap();
        let cmd = cli.command.unwrap_or(Commands::Read);
        let op = map_command(cmd).unwrap().unwrap();
        assert_eq!(op, Operation::Read);
    }

    #[test]
    fn test_cli_speed_arg() {
        let cli = Cli::try_parse_from(["tts", "speed", "1.2"]).unwrap();
        let cmd = cli.command.unwrap();
        let op = map_command(cmd).unwrap().unwrap();
        assert_eq!(op, Operation::SetSpeed { speed: 1.2 });
    }

    #[test]
    fn test_cli_log_shortcut() {
        let cli = Cli::try_parse_from(["tts", "log"]).unwrap();
        let cmd = cli.command.unwrap();
        let op = map_command(cmd).unwrap();
        assert!(op.is_none()); // meaning we skip daemon sending
    }

    #[test]
    fn test_cli_say_no_args_error() {
        let cli = Cli::try_parse_from(["tts", "say"]).unwrap();
        let cmd = cli.command.unwrap();
        let err = map_command(cmd).unwrap_err();
        assert_eq!(err.code, vroca_core::operations::ErrorCode::InvalidRequest);
    }
}
