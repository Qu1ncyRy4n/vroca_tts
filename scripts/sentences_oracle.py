"""Print Python's sentences() output for each input as a Rust test table row.

The oracle for rust_impl/crates/vroca-core/tests/text_parity.rs. Only the
splitter's source is executed, so no daemon dependencies are imported.

Usage: python3 -I scripts/sentences_oracle.py python_impl/daemon.py
"""
import json
import re
import sys

src = open(sys.argv[1]).read()
start = src.index("BOUNDARY = ")
end = src.index("class Mpv")
ns = {"re": re}
exec(src[start:end], ns)
sentences = ns["sentences"]

INPUTS = [
    "",
    "   ",
    "Hello world.",
    "Hello world. How are you? Fine!",
    "No terminal punctuation",
    "Dr. Smith went home. He slept.",
    "Mr. and Mrs. Jones arrived. They sat.",
    "J. R. R. Tolkien wrote books. Yes.",
    "Use e.g. this one. And i.e. that one. Done.",
    "Pi is 3.14 roughly. Yes.",
    "Wow!! Really?! Okay.",
    'He said "hi." Then he left.',
    "(This is aside.) Next sentence.",
    "End with spaces.   ",
    "# Title\nBody text here",
    "## Sub heading\n- item one\n- item two",
    "1. first\n2. second",
    "> quoted line\nAfter.",
    "Some `inline code` here.",
    "Before ```\ncode block\n``` after.",
    "Unclosed ``` fence here.",
    "A [link](http://x.y) here.",
    "An ![alt text](img.png) here.",
    "**bold** and *em* and __strong__ and ~~gone~~ text.",
    "snake_case_name is used.",
    "**",
    "Tabs\tand\nnewlines\n\nare collapsed.",
    "Ends with abbrev etc. and continues.",
    "Ellipsis... then more. Fin.",
    "Initial A. Smith spoke. Then B left.",
    "Quote 'single.' Next.",
    "Bracket [note.] Next.",
    "####### seven hashes",
    "#nospace heading",
    "* star bullet\n+ plus bullet",
    "Unicode café. Naïve résumé!",
]

for text in INPUTS:
    print(f"        ({json.dumps(text)}, &{json.dumps(sentences(text))}),")
