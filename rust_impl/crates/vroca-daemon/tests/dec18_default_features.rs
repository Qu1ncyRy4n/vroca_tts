//! DEC-18: with default features, `vroca-daemon`, `vroca-client`, and the
//! `vroca` CLI contain no socket, child-process, or systemd code. All of it
//! lives in a `transport.rs` module that each crate root declares only under
//! `#[cfg(feature = "transport")]`. This scans the sources, so it holds for
//! every feature combination and fails the moment transport code leaks out.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

const FORBIDDEN: [&str; 5] = [
    "std::os::unix::net",
    "UnixListener",
    "UnixStream",
    "process::Command",
    "journalctl",
];

fn crate_src(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn check(src: &Path, root: &str) {
    let root_text = std::fs::read_to_string(src.join(root)).unwrap();
    let gated = root_text.lines().collect::<Vec<_>>().windows(2).any(|w| {
        w.first().map(|l| l.trim()) == Some("#[cfg(feature = \"transport\")]")
            && w.get(1)
                .is_some_and(|l| l.trim_end().ends_with("mod transport;"))
    });
    assert!(
        gated,
        "{}/{root} must declare `mod transport;` under #[cfg(feature = \"transport\")]",
        src.display()
    );

    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs")
            || path.file_name().is_some_and(|n| n == "transport.rs")
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for needle in FORBIDDEN {
            assert!(
                !text.contains(needle),
                "{} mentions `{needle}` outside transport.rs (DEC-18)",
                path.display()
            );
        }
    }
}

#[test]
fn daemon_transport_is_gated() {
    check(&crate_src("src"), "lib.rs");
}

#[test]
fn client_transport_is_gated() {
    check(&crate_src("../vroca-client/src"), "lib.rs");
}

#[test]
fn cli_transport_is_gated() {
    check(&crate_src("../../bins/vroca/src"), "main.rs");
}
