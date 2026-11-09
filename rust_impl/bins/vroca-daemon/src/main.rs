#![forbid(unsafe_code)]

use std::process::ExitCode;

#[cfg(feature = "transport")]
fn main() -> ExitCode {
    use vroca_daemon::player::FakePlayer;
    use vroca_daemon::transport::{Daemon, DaemonConfig};

    let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") else {
        eprintln!("vroca-daemon: XDG_RUNTIME_DIR is not set");
        return ExitCode::FAILURE;
    };
    // No engine or real player exists until slice 6.
    eprintln!("vroca-daemon: fake player, no audio");
    let (player, _handle) = FakePlayer::new();
    match Daemon::start(
        DaemonConfig {
            runtime_dir: runtime_dir.into(),
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
