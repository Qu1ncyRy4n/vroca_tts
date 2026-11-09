//! Sentence-splitter parity with `python_impl/daemon.py` `sentences()`.
//!
//! Every expected value below was produced by running the Python function
//! itself on the input (2026-11-08), not written by hand:
//! `nix develop -c python3 -I scripts/sentences_oracle.py python_impl/daemon.py`
//! (rewrite its JSON `é` escapes as literal characters). Regenerate after
//! any change to the Python splitter.

use vroca_core::text::sentences;

#[rustfmt::skip]
const PARITY: &[(&str, &[&str])] = &[
    ("", &[]),
    ("   ", &[]),
    ("Hello world.", &["Hello world."]),
    ("Hello world. How are you? Fine!", &["Hello world.", "How are you?", "Fine!"]),
    ("No terminal punctuation", &["No terminal punctuation"]),
    ("Dr. Smith went home. He slept.", &["Dr. Smith went home.", "He slept."]),
    ("Mr. and Mrs. Jones arrived. They sat.", &["Mr. and Mrs. Jones arrived.", "They sat."]),
    ("J. R. R. Tolkien wrote books. Yes.", &["J. R. R. Tolkien wrote books.", "Yes."]),
    ("Use e.g. this one. And i.e. that one. Done.", &["Use e.g. this one.", "And i.e. that one.", "Done."]),
    ("Pi is 3.14 roughly. Yes.", &["Pi is 3.14 roughly.", "Yes."]),
    ("Wow!! Really?! Okay.", &["Wow!!", "Really?!", "Okay."]),
    ("He said \"hi.\" Then he left.", &["He said \"hi.\"", "Then he left."]),
    ("(This is aside.) Next sentence.", &["(This is aside.)", "Next sentence."]),
    ("End with spaces.   ", &["End with spaces."]),
    ("# Title\nBody text here", &["Title.", "Body text here"]),
    ("## Sub heading\n- item one\n- item two", &["Sub heading.", "item one.", "item two."]),
    ("1. first\n2. second", &["first.", "second."]),
    ("> quoted line\nAfter.", &["quoted line.", "After."]),
    ("Some `inline code` here.", &["Some inline code here."]),
    ("Before ```\ncode block\n``` after.", &["Before Code snippet.", "after."]),
    ("Unclosed ``` fence here.", &["Unclosed ``` fence here."]),
    ("A [link](http://x.y) here.", &["A link here."]),
    ("An ![alt text](img.png) here.", &["An !alt text here."]),
    ("**bold** and *em* and __strong__ and ~~gone~~ text.", &["bold and em and strong and gone text."]),
    ("snake_case_name is used.", &["snakecasename is used."]),
    ("**", &[]),
    ("Tabs\tand\nnewlines\n\nare collapsed.", &["Tabs and newlines are collapsed."]),
    ("Ends with abbrev etc. and continues.", &["Ends with abbrev etc. and continues."]),
    ("Ellipsis... then more. Fin.", &["Ellipsis...", "then more.", "Fin."]),
    ("Initial A. Smith spoke. Then B left.", &["Initial A. Smith spoke.", "Then B left."]),
    ("Quote 'single.' Next.", &["Quote 'single.'", "Next."]),
    ("Bracket [note.] Next.", &["Bracket [note.]", "Next."]),
    ("####### seven hashes", &["####### seven hashes"]),
    ("#nospace heading", &["#nospace heading"]),
    ("* star bullet\n+ plus bullet", &["star bullet.", "plus bullet."]),
    ("Unicode café. Naïve résumé!", &["Unicode café.", "Naïve résumé!"]),
];

#[test]
fn sentences_match_python() {
    let mut failures = Vec::new();
    for (input, want) in PARITY {
        let got = sentences(input);
        if got != *want {
            failures.push(format!(
                "input {input:?}\n   want {want:?}\n    got {got:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} rows differ from Python:\n{}",
        failures.len(),
        PARITY.len(),
        failures.join("\n")
    );
}
