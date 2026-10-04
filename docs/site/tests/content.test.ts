import { describe, expect, it } from "vitest";
import {
	counts,
	pluginOf,
	searchSkills,
	skillById,
	triggers,
} from "../src/content";

describe("generated content", () => {
	it("covers every plugin and skill", () => {
		expect(counts.plugins).toBeGreaterThanOrEqual(9);
		expect(counts.skills).toBeGreaterThanOrEqual(36);
	});

	it("finds a skill by the words of a task", () => {
		const results = searchSkills("load test");
		expect(results.some((s) => s.id === "qa-load")).toBe(true);
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
});
