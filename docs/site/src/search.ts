// Search over the build-time index (src/content/search-index.json, written by
// scripts/generate-search-index.ts). A pure function, so it is tested without
// React or a DOM: tests/search.test.ts holds the query table that is its
// contract. No network, no dependency.
//
// Score = for each query term, the sum over the entry's fields of
// field weight x match quality; every term must hit some field.
//   exact word 1.0 | word prefix 0.7 | one edit (terms of 4+ letters) 0.4
// Ties: skills before pages before headings, then id.

export type EntryKind = "skill" | "page" | "heading";

export interface SearchEntry {
	kind: EntryKind;
	id: string;
	/** Skill name, page title or heading text. */
	title: string;
	/** `/plugin:skill` for a skill. */
	command?: string;
	plugin?: string;
	/** The description, or a page's lede. */
	text: string;
	/** The quoted "Use for ..." phrases of a skill description. */
	triggers?: string;
	/** Scripts a skill is built around (`wt.py`), as words. */
	tools?: string;
	/** Headings of a page, as one string. */
	headings?: string;
	/** Route inside the site. */
	url: string;
}

/** What the build writes: a skill is just its id (and scripts); the site
 *  completes it from the skill data. Pages and headings are complete. */
export type StoredEntry =
	| (Pick<SearchEntry, "id" | "tools"> & { kind: "skill" })
	| (SearchEntry & { kind: "page" | "heading" });

export const WEIGHTS = {
	name: 10,
	id: 8,
	triggers: 3,
	plugin: 4,
	headings: 4,
	tools: 3,
	text: 1,
} as const;

const PREFIX = 0.7;
const TYPO = 0.4;
const KIND_ORDER: Record<EntryKind, number> = { skill: 0, page: 1, heading: 2 };

export function words(s: string): string[] {
	return s.toLowerCase().match(/[\p{L}\p{N}]+/gu) ?? [];
}

/** True when a and b differ by at most one insertion, deletion, substitution
 *  or swap of adjacent letters. */
export function withinOneEdit(a: string, b: string): boolean {
	if (a === b) return true;
	const d = a.length - b.length;
	if (d > 1 || d < -1) return false;
	let i = 0;
	while (i < a.length && i < b.length && a[i] === b[i]) i++;
	if (a.length === b.length) {
		if (a.slice(i + 1) === b.slice(i + 1)) return true; // substitution
		return (
			a[i] === b[i + 1] &&
			a[i + 1] === b[i] &&
			a.slice(i + 2) === b.slice(i + 2)
		); // swap
	}
	const [long, short] = a.length > b.length ? [a, b] : [b, a];
	return long.slice(i + 1) === short.slice(i);
}

function quality(term: string, tokens: string[]): number {
	let best = 0;
	for (const w of tokens) {
		if (w === term) return 1;
		if (w.startsWith(term)) best = Math.max(best, PREFIX);
		else if (
			best < TYPO &&
			term.length >= 4 &&
			(withinOneEdit(term, w) ||
				// a typo inside the part of a longer word the term covers
				(w.length > term.length &&
					withinOneEdit(term, w.slice(0, term.length))))
		)
			best = TYPO;
	}
	return best;
}

interface Prepared {
	entry: SearchEntry;
	fields: [number, string[]][];
}

function prepare(entry: SearchEntry): Prepared {
	return {
		entry,
		fields: [
			[WEIGHTS.name, words(`${entry.title} ${entry.command ?? ""}`)],
			[WEIGHTS.id, words(entry.id)],
			[WEIGHTS.plugin, words(entry.plugin ?? "")],
			[WEIGHTS.triggers, words(entry.triggers ?? "")],
			[WEIGHTS.headings, words(entry.headings ?? "")],
			[WEIGHTS.tools, words(entry.tools ?? "")],
			[WEIGHTS.text, words(entry.text)],
		],
	};
}

export function searchIndex(
	index: SearchEntry[],
	query: string,
	limit = 50,
): SearchEntry[] {
	const terms = words(query);
	if (terms.length === 0) return [];
	const scored: { entry: SearchEntry; score: number }[] = [];
	for (const p of index.map(prepare)) {
		let total = 0;
		let all = true;
		for (const term of terms) {
			let s = 0;
			for (const [weight, tokens] of p.fields)
				s += weight * quality(term, tokens);
			if (s === 0) {
				all = false;
				break;
			}
			total += s;
		}
		if (all) scored.push({ entry: p.entry, score: total });
	}
	return scored
		.sort(
			(a, b) =>
				b.score - a.score ||
				KIND_ORDER[a.entry.kind] - KIND_ORDER[b.entry.kind] ||
				a.entry.id.localeCompare(b.entry.id),
		)
		.slice(0, limit)
		.map((r) => r.entry);
}
