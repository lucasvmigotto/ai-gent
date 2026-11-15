import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// SC-007: releases are tagged and written without a "v" prefix.
const root = join(import.meta.dirname, "..");
const SKIP = new Set(["generated.json", "search-index.json"]);

function files(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (SKIP.has(name)) return [];
		return statSync(path).isDirectory() ? files(path) : [path];
	});
}

describe("release names", () => {
	it("writes no release with a v prefix under src or scripts", () => {
		const found = [...files(join(root, "src")), ...files(join(root, "scripts"))]
			.filter((f) => /\.(ts|tsx|mjs|svg)$/.test(f))
			.flatMap((f) =>
				readFileSync(f, "utf8")
					.split("\n")
					.map((line, i) => ({ f, i: i + 1, line }))
					.filter(({ line }) => /(?<![A-Za-z])v\d+\.\d+\.\d+|@v\d/.test(line))
					.map(({ f, i }) => `${f.replace(root, "")}:${i}`),
			);
		expect(found).toEqual([]);
	});
});
