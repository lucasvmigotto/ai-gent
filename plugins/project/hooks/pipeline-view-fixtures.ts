// Snapshots the pane's view tests draw, built from the README fixture the same
// way pipeline.ts builds a feature (pure: no `$`). The compatibility goldens in
// pipeline-golden.ts are 013's `view` and `text` output for them.

import type { ArtifactState, Feature, Snapshot } from '../types'
import { README_TEXT } from './pipeline-fixtures'
import { ARTIFACTS, featureState, parseReadme, recommend, split } from './pipeline-model'

export function featuresOf(readme: string): Feature[] {
  const rows = parseReadme(readme)
  const plan = split(rows)
  return rows.map(row => ({
    number: row.number,
    name: row.name,
    priority: row.priority,
    depends: row.depends,
    sides: row.sides,
    state: featureState(row.sides, row.notes),
    bucket: plan.done.includes(row.number) ? 'done' : plan.blocked.includes(row.number) ? 'blocked' : 'ready',
    waiting: plan.waiting[row.number] ?? [],
    files: { spec: true, plan: row.number < '004', tasks: row.number < '003' },
  }))
}

// brief, specs index (older than the brief) and ux-vision present; architecture
// missing although the specs index ran; the rest pending.
export function artifactsOf(): ArtifactState[] {
  const present = new Set(['docs/product/brief.md', 'specs/README.md', 'docs/product/ux-vision.md'])
  return ARTIFACTS.map(spec => ({
    path: spec.path,
    label: spec.label,
    stage: spec.stage,
    isPresent: present.has(spec.path),
    commitDate: present.has(spec.path) ? '2026-10-01' : null,
    staleSince: spec.path === 'specs/README.md' ? { path: 'docs/product/brief.md', date: '2026-10-09' } : null,
    isProblem: spec.path === 'docs/product/architecture.md',
  }))
}

export function snapshotOf(readme: string = README_TEXT, patch: Partial<Snapshot> = {}): Snapshot {
  const artifacts = artifactsOf()
  const features = featuresOf(readme)
  const items = [
    { kind: 'clarification' as const, file: 'specs/004-x/spec.md', line: 12, text: 'which login?' },
    { kind: 'assumption' as const, file: 'docs/product/brief.md', line: 3, text: 'one tenant' },
  ]
  return {
    artifacts,
    features,
    hasReadme: true,
    markers: { counts: { gap: 2, clarification: 1, assumption: 1 }, items, isTruncated: false },
    next: recommend(artifacts, features, items),
    hasGit: true,
    computedAt: 0,
    ...patch,
  }
}

// A README of `count` rows, each ready or done by turns, for the row cap.
export function bigReadme(count: number): string {
  const rows = Array.from({ length: count }, (_, i) => {
    const n = String(i + 1).padStart(3, '0')
    return `| ${n} | feature-number-${n}-with-a-long-name-to-cut | x | P${(i % 3) + 1} | — | ${i % 2 ? 'Implemented' : 'Planned'} | N/A | — | |`
  })
  return `| # | Feature | Scope | Priority | Depends on | Frontend | Backend | QA | Notes |\n|---|---|---|---|---|---|---|---|---|\n${rows.join('\n')}\n`
}
