// Parity fixture for the pipeline pane: a specs/README.md and what the two scripts
// that already read it make of it. pipeline.test.tsx checks the pane's model
// against EXPECTED; plugins/project/scripts/test_scripts.py runs specsync.py
// (a feature's state) and wt.py (ready, blocked, done) on README_TEXT and checks
// they give exactly EXPECTED. Regenerate EXPECTED from the scripts if either
// changes how it reads the README; keep it strict JSON (the Python test parses it).

export const README_TEXT = `# Features

Intro text that is not a table.

| # | Feature | Scope | Priority | Depends on | Frontend | Backend | QA | Notes |
|---|---|---|---|---|---|---|---|---|
| 001 | shell | layout | P1 | — | Implemented | N/A | — | done |
| 002 | search | find | P1 | 001 | In progress | Planned | — | |
| 003 | guides | pages | P2 | 001, 002 | Planned | N/A | — | waits on 002 |
| 004 | llm-output | llms | P1 | 001 | Implemented (tests untested) | N/A | Verified | |
| 005 | delivery | ci | P1 | — | N/A | Verified | — | |
| 006 | installer | install | P1 | — | N/A | Partial | — | |
| 007 | old | gone | P3 | — | Retired | Retired | — | superseded by 002 |
| 008 | late | x | P2 | 005 | — | Planned | — | |
| 009 | blocked-on-partial | x | P2 | 006 | N/A | Planned | — | |
| abc | not a feature row | | | | | | | |
`

export const EXPECTED = {
  "states": {
    "001": "review",
    "002": "todo",
    "003": "todo",
    "004": "review",
    "005": "done",
    "006": "doing",
    "007": "retired",
    "008": "todo",
    "009": "todo"
  },
  "ready": [
    "002",
    "006",
    "008"
  ],
  "blocked": [
    "003",
    "009"
  ],
  "done": [
    "001",
    "004",
    "005",
    "007"
  ]
}
