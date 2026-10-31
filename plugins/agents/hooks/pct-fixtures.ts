// The contract between `errorPercent` (landing-model.ts) and token_report.py's
// `pct_error` (specs/019-card-cost-on-landing). Plain data in a fixed shape:
// test_token_report.py reads the rows below by pattern, landing.test.tsx imports
// them, so both sides answer the same table.

// [projected, used, expected] over the ledger cells' parsed (already rounded)
// values, rounded half to even like Python's `{:+.0f}`. The exact .5 rows
// (0.5, 1.5, 2.5 and their negatives) are where half-up and half-even part ways.
export const PCT_FIXTURES: readonly [number, number, string][] = [
  [420000, 391000, '-7%'],
  [420000, 550000, '+31%'],
  [420000, 240000, '-43%'],
  [420000, 420000, '+0%'],
  [300000, 302000, '+1%'],
  [1240000, 1410000, '+14%'],
  [200000, 201000, '+0%'],
  [400000, 406000, '+2%'],
  [400000, 410000, '+2%'],
  [8000, 9000, '+12%'],
  [200000, 199000, '-0%'],
  [200000, 197000, '-2%'],
  [400000, 390000, '-2%'],
  [400000, 396000, '-1%'],
  [400000, 401000, '+0%'],
]

// A ledger-cell pair and the exact figures behind it: tracker.md's Error column
// is computed from the exact figures, the toast from the cells, so they may differ
// by a point. [cell projected, cell used, cell %, exact projected, exact used, exact %].
export const PCT_GAP_FIXTURES: readonly [number, number, string, number, number, string][] = [
  [400000, 410000, '+2%', 399600, 410400, '+3%'],
]
