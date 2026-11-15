import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { staticPages } from "../scripts/llms-pages";
import { installFacts } from "../src/content/install-facts";
import { enUS } from "../src/i18n";

// FR-003: the Install page's commands are the repository's. Every fact must be
// backed by its source file, and every command the Markdown twin shows must
// appear in the README, in install.sh, or be a fact.
const repo = join(import.meta.dirname, "../../..");
const read = (path: string) => readFileSync(join(repo, path), "utf8");
const norm = (s: string) => s.replace(/\s+/g, " ").trim();

const facts = Object.values(installFacts);
const factCommands = new Set(
	facts.flatMap((f) => f.command.split("\n").map(norm)),
);
const documented = norm(
	[read("README.md"), read("install.sh")]
		.join("\n")
		.split("\n")
		.map((l) => l.replace(/\s+#\s.*$/, ""))
		.join(" "),
);

function twinCommands(): string[] {
	const body = staticPages("en-US").find((p) => p.slug === "install")?.body;
	const out: string[] = [];
	let fenced = false;
	for (const line of (body ?? "").split("\n")) {
		if (line.startsWith("```")) {
			fenced = !fenced;
			continue;
		}
		if (fenced && line.trim()) out.push(norm(line));
	}
	return out;
}

describe("install page drift guard", () => {
	for (const f of facts) {
		it(`${f.id}: ${f.source} still carries it`, () => {
			const text = read(f.source);
			for (const needle of f.needles) expect(text).toContain(needle);
		});
	}

	it("shows only commands that are facts or documented in README.md or install.sh", () => {
		const loose = twinCommands().filter(
			(c) => !factCommands.has(c) && !documented.includes(c),
		);
		// Troubleshooting commands (./setup.sh ...) are checked flag by flag below.
		expect(loose.filter((c) => !c.startsWith("./setup.sh"))).toEqual([]);
	});

	it("uses only setup.sh flags that setup.sh defines", () => {
		const setup = read("setup.sh");
		const flags = twinCommands()
			.filter((c) => c.startsWith("./setup.sh"))
			.flatMap((c) => c.match(/--[a-z-]+/g) ?? []);
		expect(flags.length).toBeGreaterThan(0);
		for (const flag of flags) expect(setup).toContain(flag);
	});

	it("keeps every fact command in the page", () => {
		const shown = new Set(twinCommands());
		const missing = facts
			.flatMap((f) => f.command.split("\n").map(norm))
			.filter((c) => !shown.has(c));
		expect(missing).toEqual([]);
	});

	it("covers the install, pin, rollback and verify commands (SC-002)", () => {
		const shown = twinCommands().join("\n");
		for (const needle of [
			"install.sh | sh",
			"VERSION=",
			"--rollback",
			"--verify-attestation",
			"gh attestation verify",
		])
			expect(shown).toContain(needle);
		expect(enUS.install.troubleshootingItems.length).toBeGreaterThanOrEqual(4);
	});
});
