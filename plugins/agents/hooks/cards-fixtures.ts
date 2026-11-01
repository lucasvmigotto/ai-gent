// A ledger with the optional Branch and Worktree columns and a `token_report.py
// tasks --dry-run --json` output over it: A and B overlap, C is alone, the
// queued and review cards have no span, and one record is for a card the
// ledger does not list. Shared by the tests; the keys are the script's own.

export const CARDS_LEDGER = `# Orchestration ledger

| Card | State | Branch | Worktree | Tokens |
|---|---|---|---|---|
| \`004-us1-core\` | running | feat/us1 | .worktrees/us1 | ~420k / 391k |
| \`004-us2\` | running | feat/us2 | .worktrees/us2 | ~300k / — |
| \`004-us3\` | review | feat/us3 | .worktrees/us3 | ~200k / 180k |
| \`004-us4\` | queued | — | — | — / — |
| \`004-us5\` | landed | feat/us5 | .worktrees/us5 | ~100k / 90k |
`

export const CARDS_LEDGER_PLAIN = `| Card | State |
|---|---|
| \`a\` | running |
| \`b\` | weird |
`

const record = (card: string, start: string, end: string, used: number) => ({
  card,
  feature: '004-x',
  model: 'sonnet',
  projected: 100_000,
  used,
  start,
  end,
  agents: 2,
  calls: 10,
})

export const CARDS_MEASURE = {
  cards: [
    record('004-us1-core', '2026-10-31T14:00:00Z', '2026-10-31T14:40:00Z', 391_000),
    record('004-us1-core', '2026-10-31T14:30:00Z', '2026-10-31T14:45:00Z', 20_000), // a fix round: one span
    record('004-us2', '2026-10-31T14:10:00Z', '2026-10-31T14:50:00Z', 250_000),
    record('004-us5', '2026-10-31T15:00:00Z', '2026-10-31T15:20:00Z', 90_000),
    record('old-card', '2026-10-30T09:00:00Z', '2026-10-30T09:10:00Z', 5_000), // not in the ledger
  ],
  unattributed: 1,
}
