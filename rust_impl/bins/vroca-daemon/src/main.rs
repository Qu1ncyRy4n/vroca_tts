#![forbid(unsafe_code)]

use std::process::ExitCode;

/// daemon.py `CONFIG_DIR`: `$XDG_CONFIG_HOME/vroca`, else `~/.config/vroca`,
/// reading the pre-rename `tts` directory until it has been moved.
#[cfg(feature = "transport")]
fn prefs_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    let current = config_home.join("vroca");
    let legacy = config_home.join("tts");
    if !current.is_dir() && legacy.is_dir() {
        Some(legacy)
    } else {
        Some(current)
    }
}

#[cfg(feature = "transport")]
fn main() -> ExitCode {
    use vroca_daemon::player::FakePlayer;
    use vroca_daemon::transport::{Daemon, DaemonConfig};

    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        eprintln!("vroca-daemon: XDG_RUNTIME_DIR is not set");
        return ExitCode::FAILURE;
    };
    let prefs_dir = prefs_dir();
    if prefs_dir.is_none() {
        eprintln!(
            "vroca-daemon: neither XDG_CONFIG_HOME nor HOME is set; preferences will not be saved"
        );
    }
    // No engine or real player exists until slice 6.
    eprintln!("vroca-daemon: fake player, no audio");
    let (player, _handle) = FakePlayer::new();
    match Daemon::start(
        DaemonConfig {
            runtime_dir: runtime_dir.into(),
            prefs_dir,
        },
        Box::new(player),
    ) {
        Ok(daemon) => {
            daemon.wait();
            // `quit` exits 0 so Restart=on-failure leaves the service
            // stopped (decided 2026-11-08, rust-spec.md §7.13).
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("vroca-daemon: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "transport"))]
fn main() -> ExitCode {
    eprintln!("vroca-daemon: built without the `transport` feature (rust-spec.md DEC-18)");
    ExitCode::FAILURE
}
