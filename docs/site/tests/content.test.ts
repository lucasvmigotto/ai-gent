import { describe, expect, it } from "vitest";
import {
	changelogVersions,
	counts,
	groupChangelog,
	pluginOf,
	searchSkills,
	skillById,
	skills,
	sourceUrl,
	triggers,
} from "../src/content";

describe("generated content", () => {
	it("generates a complete, unique page per skill", () => {
		const ids = skills.map((s) => s.id);
		expect(new Set(ids).size).toBe(ids.length);
		for (const s of skills) {
			expect(s.description.trim().length).toBeGreaterThan(0);
			expect(s.body.trim().length).toBeGreaterThan(0);
			expect(s.sourceUrl).toMatch(/^https:\/\/github\.com\//);
		}
	});

	it("covers every plugin and skill", () => {
		expect(counts.plugins).toBeGreaterThanOrEqual(9);
		expect(counts.skills).toBeGreaterThanOrEqual(36);
	});

	it("finds a skill by the words of a task", () => {
		const results = searchSkills("load test");
		expect(results.some((s) => s.id === "qa-load")).toBe(true);
	});

	it("finds a skill by its Claude Code command", () => {
		expect(searchSkills("qa:load")[0]?.id).toBe("qa-load");
	});

	it("links sources to the ref the site was built from, not main", () => {
		const skill = skillById("qa-load");
		if (!skill) throw new Error("missing qa-load skill");
		expect(skill.sourceUrl).not.toContain("/blob/main/");
		expect(skill.sourceUrl).toBe(sourceUrl("plugins/qa/skills/load/SKILL.md"));
	});

	it("returns nothing for an empty query", () => {
		expect(searchSkills("   ")).toHaveLength(0);
	});

	it("links a skill to its plugin", () => {
		const skill = skillById("qa-load");
		expect(skill).toBeDefined();
		if (!skill) throw new Error("missing qa-load skill");
		expect(pluginOf(skill)?.name).toBe("qa");
	});

	it("extracts trigger phrases from a description", () => {
		const skill = skillById("qa-load");
		if (!skill) throw new Error("missing qa-load skill");
		const list = triggers(skill.description);
		expect(list.length).toBeGreaterThan(0);
		expect(list.some((w) => w.includes("load test"))).toBe(true);
	});

	it("parses every changelog heading with a non-empty body", () => {
		expect(changelogVersions.length).toBeGreaterThan(0);
		for (const e of changelogVersions) {
			expect(e.body.trim().length).toBeGreaterThan(0);
			expect(e.body).not.toMatch(/^## /m);
		}
	});

	it("gives a dated version its major, minor and patch numbers", () => {
		const released = changelogVersions.find((e) => e.version === "1.0.0");
		expect(released).toMatchObject({ major: 1, minor: 0, patch: 0 });
		expect(released?.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
	});

	it("leaves Unreleased's version parts null", () => {
		const unreleased = changelogVersions.find(
			(e) => e.version === "Unreleased",
		);
		if (unreleased) {
			expect(unreleased).toMatchObject({
				major: null,
				minor: null,
				patch: null,
				date: null,
			});
		}
	});

	it("folds released entries by major, then minor, newest first", () => {
		const released = changelogVersions.filter(
			(e) => e.version !== "Unreleased",
		);
		const majors = groupChangelog(released);
		expect(majors.length).toBeGreaterThan(0);
		for (let i = 1; i < majors.length; i++) {
			const prev = majors[i - 1];
			const curr = majors[i];
			if (!prev || !curr) throw new Error("index out of bounds");
			expect(prev.major).toBeGreaterThan(curr.major);
		}
		const [first] = majors;
		if (!first) throw new Error("no major group parsed");
		for (let i = 1; i < first.minors.length; i++) {
			const prev = first.minors[i - 1];
			const curr = first.minors[i];
			if (!prev || !curr) throw new Error("index out of bounds");
			expect(prev.minor).toBeGreaterThan(curr.minor);
		}
		// every entry under a minor actually belongs to it
		for (const g of majors) {
			for (const m of g.minors) {
				for (const e of m.entries) {
					expect(e.major).toBe(g.major);
					expect(e.minor).toBe(m.minor);
				}
			}
		}
	});
});
