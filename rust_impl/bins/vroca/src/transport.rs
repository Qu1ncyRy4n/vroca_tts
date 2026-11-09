//! The CLI's socket round trip and the `log` shortcut. Built only with the
//! `transport` feature (DEC-18). Slice D replaces the `/tmp` fallback
//! (review bug 13) and the `unavailable` message (§7.2).

use std::process::{Command, ExitCode};

use vroca_client::Client;
use vroca_core::codec::Response;
use vroca_core::operations::Operation;

pub fn log() -> ExitCode {
    let status = Command::new("journalctl")
        .args(["--user", "-u", "vroca", "-u", "vroca-overlay", "-f"])
        .status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(e) => eprintln!("error: could not render reply: {e}"),
    }
}

pub fn send(op: Operation) -> ExitCode {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket_path = std::path::PathBuf::from(runtime_dir).join("vroca-v1.sock");

    let client = Client::new(socket_path);
    match client.send(op) {
        Ok(Response::Ok) => ExitCode::SUCCESS,
        Ok(Response::String { data }) => {
            println!("{data}");
            ExitCode::SUCCESS
        }
        Ok(Response::Status { snapshot }) => {
            print_json(&snapshot);
            ExitCode::SUCCESS
        }
        Ok(Response::Catalogue { data }) => {
            print_json(&data);
            ExitCode::SUCCESS
        }
        Ok(Response::Error { error }) | Err(error) => {
            eprintln!("error: {}", error.message);
            ExitCode::from(error.exit_code())
        }
    }
}
