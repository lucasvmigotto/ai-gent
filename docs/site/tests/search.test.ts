import { describe, expect, it } from "vitest";
import { search } from "../src/content";
import { type SearchEntry, searchIndex, withinOneEdit } from "../src/search";

// The contract of the ranking (spec 040 FR-007, SC-004): the first result for
// each query. Change a weight in src/search.ts only if this table still reads
// right; edit the table when the intent changes.
// Each row: the query, the kind of the first result, and its title (headings,
// pages) or id (skills). An array means any of those (equal scores break by id).
const FIRST: [
	query: string,
	kind: SearchEntry["kind"],
	who: string | string[],
][] = [
	["commit", "skill", "git-workflow"],
	["instal", "page", "Install ai-gent"],
	["install", "page", "Install ai-gent"],
	["worktree", "heading", "Worktree tool (wt.py)"],
	["wt", "heading", "Worktree tool (wt.py)"],
	["token", "heading", "Token tracking (ai-gent tokens)"],
	["secret", "skill", "secrets-pass"],
	["database", "skill", "db-connect"],
	["release", "heading", "Releases"],
	["pr", "skill", "repo-pr"],
	["spec", "skill", ["backend-spec", "frontend-spec", "project-spec"]],
	["compact", "skill", "compact-code"],
	["orchestrate", "skill", "agents-orchestrate"],
	["checkpoint", "skill", "project-checkpoint"],
	["tier", "heading", "Tiers and agents"],
	["rollback", "page", "Install ai-gent"],
	["roll back", "heading", "Update and roll back"],
	["qa:load", "skill", "qa-load"],
	["devcontainer", "skill", "devcontainer-setup"],
];

const label = (e: SearchEntry | undefined) =>
	e?.kind === "skill" ? e.id : e?.title;

describe("search ranking", () => {
	it("covers at least 15 queries", () => {
		expect(FIRST.length).toBeGreaterThanOrEqual(15);
	});

	for (const [query, kind, who] of FIRST) {
		it(`"${query}" first finds ${[who].flat().join(" or ")}`, () => {
			const first = search(query)[0];
			expect(first?.kind).toBe(kind);
			expect([who].flat()).toContain(label(first));
		});
	}

	it("ranks a command or name hit above a description-only hit", () => {
		const ids = search("commit").map((e) => e.id);
		expect(ids.indexOf("git-workflow")).toBe(0);
	});

	it("finds the Install page by a prefix and by a typo", () => {
		expect(search("instal").some((e) => e.id === "install")).toBe(true);
		expect(search("instll").some((e) => e.id === "install")).toBe(true);
	});

	it("shows the worktree skill and the compatibility row for wt", () => {
		const hits = search("wt");
		expect(hits.some((e) => e.id === "agents-parallel")).toBe(true);
		expect(
			hits.some((e) => e.kind === "heading" && e.url === "/use/compatibility"),
		).toBe(true);
	});

	it("requires every term to hit", () => {
		expect(search("commit zzzzqq")).toEqual([]);
	});

	it("returns nothing for an empty query or only punctuation", () => {
		expect(search("")).toEqual([]);
		expect(search("  ?! ")).toEqual([]);
	});

	it("is deterministic on ties: kind, then id", () => {
		const idx: SearchEntry[] = [
			{ kind: "heading", id: "b", title: "same", text: "", url: "/" },
			{ kind: "skill", id: "z", title: "same", text: "", url: "/" },
			{ kind: "skill", id: "a", title: "same", text: "", url: "/" },
		];
		expect(searchIndex(idx, "same").map((e) => e.id)).toEqual(["a", "z", "b"]);
	});

	it("does not apply typo tolerance to short terms", () => {
		const idx: SearchEntry[] = [
			{ kind: "skill", id: "x", title: "git", text: "", url: "/" },
		];
		expect(searchIndex(idx, "gat")).toEqual([]);
		expect(withinOneEdit("install", "instll")).toBe(true);
		expect(withinOneEdit("install", "isntall")).toBe(true);
		expect(withinOneEdit("install", "uninstall")).toBe(false);
	});
});
