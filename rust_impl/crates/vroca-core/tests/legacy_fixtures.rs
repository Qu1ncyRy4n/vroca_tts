//! Legacy parser fixtures, one row per case in
//! `docs/legacy-compatibility.md`.
//!
//! `row` is the request row of that document, `kind` its fixture code
//! (V valid, M malformed, R reply shape, E error path — S, the state change,
//! is tested in the state machine), and `cites` the spec identifiers that
//! motivate the expectation. A failing row prints all four so the evidence
//! is one grep away. Error messages are the exact bytes `python_impl/
//! daemon.py` sends, except where a `DEC-` citation records a deviation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vroca_core::operations::{ErrorCode, Operation, Replace, ResetGroup, Scope, Unit};
use vroca_core::parser::parse_legacy;
use vroca_core::schema::{Aligner, EngineId, OverlayMode, Position, VoiceId, VoiceRef};

#[derive(Debug, Clone, Copy)]
enum Kind {
    V,
    M,
    R,
    E,
}

enum Expect {
    Op(Operation),
    Err(ErrorCode, &'static str),
}

struct Fixture {
    row: &'static str,
    kind: Kind,
    cites: &'static str,
    input: &'static str,
    expect: Expect,
}

fn f(
    row: &'static str,
    kind: Kind,
    cites: &'static str,
    input: &'static str,
    expect: Expect,
) -> Fixture {
    Fixture {
        row,
        kind,
        cites,
        input,
        expect,
    }
}

fn op(op: Operation) -> Expect {
    Expect::Op(op)
}

fn arg(message: &'static str) -> Expect {
    Expect::Err(ErrorCode::InvalidArgument, message)
}

fn req(message: &'static str) -> Expect {
    Expect::Err(ErrorCode::InvalidRequest, message)
}

fn speak(text: &str, replace: Replace, voice: Option<VoiceRef>) -> Operation {
    Operation::speak(text, replace, voice)
}

fn engine(name: &str) -> EngineId {
    EngineId::new(name).unwrap()
}

fn fixtures() -> Vec<Fixture> {
    use Kind::*;
    vec![
        // --- read / no-argument verbs (DEC-13) ------------------------------
        f("read", V, "DEC-13", "read", op(Operation::Read)),
        f("read", V, "N7 bug-1", "read\n", op(Operation::Read)),
        f("read", V, "N7 bug-1", "  read  ", op(Operation::Read)),
        f(
            "read",
            M,
            "DEC-13 bug-2",
            "read xyz",
            req("unknown: read xyz"),
        ),
        f(
            "stop",
            V,
            "DEC-1",
            "stop",
            op(Operation::stop(Scope::Playback)),
        ),
        f(
            "stop",
            M,
            "DEC-13 bug-2",
            "stop now",
            req("unknown: stop now"),
        ),
        f(
            "clear",
            V,
            "DEC-1",
            "clear",
            op(Operation::stop(Scope::All)),
        ),
        f(
            "skip",
            V,
            "DEC-1 D3",
            "skip",
            op(Operation::skip(Unit::Item)),
        ),
        f("toggle", V, "DEC-13", "toggle", op(Operation::Toggle)),
        f(
            "toggle",
            M,
            "DEC-13",
            "toggle on",
            req("unknown: toggle on"),
        ),
        f("next", V, "—", "next", op(Operation::Next)),
        f("back", V, "—", "back", op(Operation::Back)),
        f("faster", V, "—", "faster", op(Operation::Faster)),
        f("slower", V, "—", "slower\n", op(Operation::Slower)),
        f("mode", V, "DEC-7", "mode", op(Operation::CycleMode)),
        f("status", V, "DEC-8", "status", op(Operation::Status)),
        f(
            "catalogue",
            V,
            "DEC-10",
            "catalogue",
            op(Operation::Catalogue),
        ),
        f("unload", V, "§7.13", "unload", op(Operation::Unload)),
        f("reload", V, "—", "reload", op(Operation::Reload)),
        f("quit", V, "DEC-9 N3", "quit", op(Operation::Quit)),
        // --- say / speak / queue (DEC-1) ------------------------------------
        f(
            "say TEXT",
            V,
            "DEC-1 D2",
            "say hello world",
            op(speak("hello world", Replace::All, None)),
        ),
        f(
            "say TEXT",
            V,
            "N7",
            "say  two  spaces ",
            op(speak("two  spaces", Replace::All, None)),
        ),
        f(
            "say TEXT",
            V,
            "N7 bug-1",
            "say hello\n",
            op(speak("hello", Replace::All, None)),
        ),
        f("say TEXT", M, "N7", "say", req("say needs text")),
        f("say TEXT", M, "N7", "say   ", req("say needs text")),
        f(
            "say TEXT",
            V,
            "DEC-10 --voice",
            "say --voice af_kore hi there",
            op(speak(
                "hi there",
                Replace::All,
                Some(VoiceRef::Name("af_kore".into())),
            )),
        ),
        f(
            "say TEXT",
            V,
            "DEC-10 --voice",
            "say --voice kokoro:af_kore hi",
            op(speak(
                "hi",
                Replace::All,
                Some(VoiceRef::Qualified(VoiceId("kokoro:af_kore".into()))),
            )),
        ),
        f(
            "say TEXT",
            V,
            "DEC-10 --voice",
            "say --voice 3 hi",
            op(speak("hi", Replace::All, Some(VoiceRef::Index(3)))),
        ),
        f(
            "say TEXT",
            M,
            "--voice no body",
            "say --voice af_kore",
            req("say needs text"),
        ),
        f(
            "say TEXT",
            V,
            "--voice only at start",
            "say hello --voice x",
            op(speak("hello --voice x", Replace::All, None)),
        ),
        f(
            "speak TEXT",
            V,
            "DEC-1 alias",
            "speak hello",
            op(speak("hello", Replace::All, None)),
        ),
        f("speak TEXT", M, "N7", "speak", req("speak needs text")),
        f(
            "queue TEXT",
            V,
            "DEC-1 D4",
            "queue later",
            op(speak("later", Replace::None, None)),
        ),
        f("queue TEXT", M, "N7", "queue", req("queue needs text")),
        // --- speed (D8) ------------------------------------------------------
        f(
            "speed FLOAT",
            V,
            "D8",
            "speed 1.5",
            op(Operation::SetSpeed { speed: 1.5 }),
        ),
        f(
            "speed FLOAT",
            V,
            "bug-1",
            "speed 1.5\n",
            op(Operation::SetSpeed { speed: 1.5 }),
        ),
        f(
            "speed FLOAT",
            M,
            "D8",
            "speed abc",
            arg("speed needs a number between 0.5 and 3"),
        ),
        f(
            "speed FLOAT",
            M,
            "D8",
            "speed",
            arg("speed needs a number between 0.5 and 3"),
        ),
        f(
            "speed FLOAT",
            M,
            "D8",
            "speed 1 2",
            arg("speed needs a number between 0.5 and 3"),
        ),
        f(
            "speed FLOAT",
            M,
            "D8",
            "speed NaN",
            arg("speed needs a number between 0.5 and 3"),
        ),
        // --- voice (DEC-10, §4.1) -------------------------------------------
        f(
            "voice",
            V,
            "DEC-10 legacy index",
            "voice 3",
            op(Operation::SetVoice {
                voice: VoiceRef::Index(3),
            }),
        ),
        f(
            "voice",
            V,
            "§4.1 af_kore",
            "voice af_kore",
            op(Operation::SetVoice {
                voice: VoiceRef::Name("af_kore".into()),
            }),
        ),
        f(
            "voice",
            V,
            "DEC-10 qualified",
            "voice kokoro:af_kore",
            op(Operation::SetVoice {
                voice: VoiceRef::Qualified(VoiceId("kokoro:af_kore".into())),
            }),
        ),
        f(
            "voice",
            V,
            "unknown prefix is a name",
            "voice foo:bar",
            op(Operation::SetVoice {
                voice: VoiceRef::Name("foo:bar".into()),
            }),
        ),
        f(
            "voice",
            M,
            "D8",
            "voice",
            arg("voice needs an index or name"),
        ),
        // --- engine ----------------------------------------------------------
        f(
            "engine NAME",
            V,
            "N10",
            "engine supertonic",
            op(Operation::SetEngine {
                id: engine("supertonic"),
            }),
        ),
        f(
            "engine NAME",
            M,
            "DEC-16",
            "engine espeak",
            arg("unknown engine: espeak (have kokoro, supertonic, libritts, zipvoice, remote)"),
        ),
        f("engine NAME", M, "—", "engine", arg("engine needs a name")),
        // --- aligner ---------------------------------------------------------
        f(
            "aligner NAME",
            V,
            "—",
            "aligner energy",
            op(Operation::SetAligner {
                aligner: Aligner::Energy,
            }),
        ),
        f(
            "aligner NAME",
            V,
            "N27",
            "aligner asr",
            op(Operation::SetAligner {
                aligner: Aligner::Asr,
            }),
        ),
        f(
            "aligner NAME",
            M,
            "—",
            "aligner dtw",
            arg("unknown aligner: dtw (have asr, energy)"),
        ),
        f(
            "aligner NAME",
            M,
            "—",
            "aligner",
            arg("aligner needs a name"),
        ),
        // --- font_size / words_visible (D8, DEC-17) -------------------------
        f(
            "font_size INTEGER",
            V,
            "N19",
            "font_size 25",
            op(Operation::SetFontSize { size: 25 }),
        ),
        f(
            "font_size INTEGER",
            M,
            "D8",
            "font_size abc",
            arg("font_size needs a number between 12 and 72"),
        ),
        f(
            "font_size INTEGER",
            M,
            "D8",
            "font_size 1.5",
            arg("font_size needs a number between 12 and 72"),
        ),
        f(
            "words_visible INTEGER",
            V,
            "N20",
            "words_visible 2",
            op(Operation::SetWordsVisible { count: 2 }),
        ),
        f(
            "words_visible INTEGER",
            M,
            "D8",
            "words_visible",
            arg("words_visible needs a number between 1 and 15"),
        ),
        // --- position / mode_set --------------------------------------------
        f(
            "position NAME",
            V,
            "N21",
            "position center",
            op(Operation::SetPosition {
                pos: Position::Center,
            }),
        ),
        f(
            "position NAME",
            V,
            "N21",
            "position top",
            op(Operation::SetPosition { pos: Position::Top }),
        ),
        f(
            "position NAME",
            V,
            "N21",
            "position bottom",
            op(Operation::SetPosition {
                pos: Position::Bottom,
            }),
        ),
        f(
            "position NAME",
            M,
            "—",
            "position up",
            arg("unknown position: up"),
        ),
        f(
            "position NAME",
            M,
            "—",
            "position",
            arg("position needs a name"),
        ),
        f(
            "mode_set",
            V,
            "DEC-7 N14",
            "mode_set scroll_rsvp",
            op(Operation::SetMode {
                mode: OverlayMode::ScrollRsvp,
            }),
        ),
        f(
            "mode_set",
            V,
            "DEC-7",
            "mode_set off",
            op(Operation::SetMode {
                mode: OverlayMode::Off,
            }),
        ),
        f(
            "mode_set",
            M,
            "DEC-7",
            "mode_set sideways",
            arg("unknown mode: sideways (have subtitle, rsvp, scroll_rsvp, off)"),
        ),
        f(
            "mode_set",
            M,
            "DEC-7",
            "mode_set",
            arg("unknown mode:  (have subtitle, rsvp, scroll_rsvp, off)"),
        ),
        // --- volume / trim (R8, §10.7b) -------------------------------------
        f(
            "volume",
            V,
            "R8",
            "volume 80",
            op(Operation::SetVolume { level: 80 }),
        ),
        f(
            "volume",
            M,
            "R8 D8",
            "volume loud",
            arg("volume needs a number between 0 and 100"),
        ),
        f(
            "trim",
            V,
            "R8",
            "trim -15",
            op(Operation::SetTrim { amount: -15 }),
        ),
        f(
            "trim",
            M,
            "R8 D8",
            "trim",
            arg("trim needs a number between -100 and 100"),
        ),
        // --- reset (DEC-15) --------------------------------------------------
        f(
            "reset",
            V,
            "DEC-15 N8",
            "reset",
            op(Operation::Reset {
                group: ResetGroup::Everything,
            }),
        ),
        f(
            "reset_prefs",
            V,
            "DEC-15",
            "reset_prefs",
            op(Operation::Reset {
                group: ResetGroup::Everything,
            }),
        ),
        f(
            "reset",
            V,
            "DEC-15",
            "reset overlay",
            op(Operation::Reset {
                group: ResetGroup::Overlay,
            }),
        ),
        f(
            "reset",
            V,
            "DEC-15",
            "reset speech",
            op(Operation::Reset {
                group: ResetGroup::Speech,
            }),
        ),
        f(
            "reset",
            M,
            "DEC-15 deviates from Python `unknown: reset foo`",
            "reset foo",
            arg("unknown reset group: foo (have overlay, speech)"),
        ),
        // --- preview (DEC-14) ------------------------------------------------
        f(
            "preview",
            V,
            "DEC-14",
            "preview 1",
            op(Operation::Preview {
                voice: VoiceRef::Index(1),
            }),
        ),
        f(
            "preview",
            V,
            "DEC-14 DEC-10",
            "preview af_kore",
            op(Operation::Preview {
                voice: VoiceRef::Name("af_kore".into()),
            }),
        ),
        f("preview", M, "D8", "preview", arg("preview needs a voice")),
        // --- unknown (R: exact prefix) --------------------------------------
        f("unknown", R, "—", "frobnicate", req("unknown: frobnicate")),
        f(
            "unknown",
            R,
            "full command echoed",
            "frobnicate the thing",
            req("unknown: frobnicate the thing"),
        ),
        f("unknown", R, "N7 empty request", "", req("unknown: ")),
        f(
            "unknown",
            R,
            "N7 whitespace request",
            "   \n",
            req("unknown: "),
        ),
        f(
            "unknown",
            E,
            "case-sensitive verbs",
            "Say hi",
            req("unknown: Say hi"),
        ),
    ]
}

#[test]
fn every_legacy_fixture_parses_as_documented() {
    let mut failures = Vec::new();
    for fx in fixtures() {
        let got = parse_legacy(fx.input);
        let ok = match (&fx.expect, &got) {
            (Expect::Op(want), Ok(got)) => want == got,
            (Expect::Err(code, msg), Err(e)) => &e.code == code && e.message == *msg,
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "[{}] {:?} ({}) input={:?}\n    got: {:?}",
                fx.row, fx.kind, fx.cites, fx.input, got
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} fixture(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_fixture_row_names_a_legacy_compatibility_request() {
    // Keeps the table honest against the document it mirrors.
    let doc = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/legacy-compatibility.md"),
    )
    .expect("docs/legacy-compatibility.md");
    for fx in fixtures() {
        let verb = fx.row.split_whitespace().next().unwrap();
        // Request cells are back-ticked (some list two aliases) except the
        // literal `unknown` row.
        assert!(
            doc.contains(&format!("| `{verb}"))
                || doc.contains(&format!(", `{verb}`"))
                || doc.contains(&format!("| {verb} |")),
            "fixture row {:?} has no `{verb}` request row in legacy-compatibility.md",
            fx.row
        );
    }
}

#[test]
fn malformed_input_never_panics() {
    // D8: the whole point. Anything the socket can deliver must come back as
    // a value, including control bytes and a long request.
    let long = "say ".to_string() + &"x".repeat(1 << 16);
    for input in [
        "\0",
        "\u{7f}",
        "say \0",
        "speed \u{1}",
        "voice \u{fffd}",
        "\r\n",
        "--voice",
        &long,
    ] {
        let _ = parse_legacy(input);
    }
}
