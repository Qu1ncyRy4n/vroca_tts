//! The dependency budget, enforced.
//!
//! `rust-spec.md` DEC-2 and DEC-4 commit to a small, sync dependency graph:
//! `vroca-core` is serde only, the daemon adds one channel crate, and no crate
//! brings an async runtime. A budget nobody checks is a wish; this test is the
//! check. Raise a ceiling in the same commit that needs it, with the reason
//! written next to the number. Pattern borrowed from omnichan
//! (`crates/omni-core/tests/dep_budget.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// Total distinct packages in `Cargo.lock`, ours included.
// 2026-11-07: 48 packages (serde, serde_json, clap, crossbeam-channel, tempfile).
// Expect a deliberate raise for egui/eframe at stage 8 (omnichan measured ~366
// with glow + wayland + x11) and for the sherpa-onnx binding at stage 6.
const MAX_TRANSITIVE: usize = 60;

/// Third-party crates named in `[workspace.dependencies]`.
// 2026-11-07: serde, serde_json, crossbeam-channel, clap, tempfile.
const MAX_DIRECT: usize = 6;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn lockfile() -> String {
    std::fs::read_to_string(workspace_root().join("Cargo.lock"))
        .expect("Cargo.lock should be committed")
}

#[test]
fn the_transitive_dependency_count_stays_under_budget() {
    let n = lockfile()
        .lines()
        .filter(|l| l.trim_start().starts_with("name = "))
        .count();
    assert!(
        n <= MAX_TRANSITIVE,
        "{n} packages in Cargo.lock, budget is {MAX_TRANSITIVE}. Remove the dependency, \
         or raise MAX_TRANSITIVE in this file in the same commit, with a reason."
    );
}

#[test]
fn the_direct_dependency_count_stays_under_budget() {
    let text = std::fs::read_to_string(workspace_root().join("Cargo.toml")).unwrap();
    let mut in_table = false;
    let mut direct = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_table = t == "[workspace.dependencies]";
            continue;
        }
        if !in_table || t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((name, rest)) = t.split_once('=') {
            if !rest.contains("path =") {
                direct.push(name.trim().to_string());
            }
        }
    }
    assert!(
        direct.len() <= MAX_DIRECT,
        "{} direct dependencies ({}), budget is {MAX_DIRECT}",
        direct.len(),
        direct.join(", ")
    );
}

#[test]
fn the_forbidden_dependencies_are_absent() {
    // Each was considered and rejected for a stated reason (rust-spec.md §4,
    // DEC-4, §7.10). Adding one is a decision, not a drift.
    let lock = lockfile();
    for (name, why) in [
        (
            "tokio",
            "DEC-4: sync threads; sherpa-onnx is blocking C++ anyway",
        ),
        ("async-std", "DEC-4: no async runtime"),
        ("futures", "DEC-4: no async runtime"),
        (
            "gtk4",
            "§7.10 resolved 2026-11-07: GUI is egui/eframe, not GTK",
        ),
        (
            "anyhow",
            "typed errors with exit codes (ErrorCode::exit_code)",
        ),
        (
            "tracing",
            "std-only logging; see omnichan omni-net/log.rs pattern",
        ),
    ] {
        let needle = format!("name = \"{name}\"");
        assert!(
            !lock.contains(&needle),
            "`{name}` appeared in Cargo.lock; rejected because: {why}. If that no longer \
             holds, remove it from this list in the same commit and say why."
        );
    }
}

#[test]
fn vroca_core_depends_only_on_serde() {
    // DEC-2: core is types and pure functions. This reads the resolved lockfile
    // entry, not the manifest, so a transitive leak is caught too.
    let lock = lockfile();
    let entry = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"vroca-core\""))
        .expect("vroca-core in Cargo.lock");
    let deps: Vec<&str> = entry
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("dependencies"))
        .skip(1)
        .take_while(|l| l.trim() != "]")
        .map(|l| l.trim().trim_matches(|c| c == '"' || c == ','))
        .collect();
    for d in &deps {
        assert!(
            d.starts_with("serde"),
            "vroca-core depends on `{d}`; DEC-2 allows serde and serde_json only"
        );
    }
}
