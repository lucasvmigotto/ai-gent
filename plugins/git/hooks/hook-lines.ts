// Reading what a SessionStart command hook printed, for the mods that show it
// to the person (specs/017-repo-hygiene). The engine stores that output as a
// transcript row (door `hook-context`, origin event SessionStart) that a
// `session.append` hook can read; `classic.SessionStart`'s `next(e)` returns
// `{}` for it (verified on Claude Code 2.1.312).
//
// COPIED into plugins/{git,devcontainer}/hooks/hook-lines.ts, because the engine
// refuses an import through a symlink that leaves the plugin;
// `python3 scripts/consistency.py --sync-shared-code` rewrites the copies and
// scripts/consistency.py fails if they differ.

// The lines a SessionStart hook printed, from the text the engine stored for it
// ("SessionStart:startup hook success: <stdout>").
export function hookLines(text: string): string[] {
  const body = /hook success:\s*([\s\S]*?)(?:\n?<\/system-reminder>|$)/.exec(text)?.[1] ?? ''
  return body.split(/\r?\n/).map(line => line.trimEnd()).filter(line => line.trim() !== '')
}

// update-check.sh prints one line when a newer release tag is known.
export function updateLine(lines: readonly string[]): string | null {
  return lines.find(line => line.startsWith('[ai-gent] update available')) ?? null
}

// leftovers.sh prints a header and one line per finding.
export function leftoversBlock(lines: readonly string[]): string | null {
  const at = lines.findIndex(line => line.startsWith('ai-gent containers'))
  if (at < 0) return null
  const findings = lines.slice(at + 1).map(line => line.trim()).filter(Boolean)
  return findings.length > 0 ? `${lines[at]?.trim()} ${findings.join(' · ')}` : (lines[at]?.trim() ?? null)
}

// The text of a stored SessionStart hook row (a transcript attachment's content).
export function rowText(message: unknown): string {
  const content = (message as { content?: unknown })?.content
  return Array.isArray(content) ? content.map(part => (part as { text?: string })?.text ?? '').join('\n') : ''
}
