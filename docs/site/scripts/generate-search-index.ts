// Builds src/content/search-index.json: every skill, every static page and its
// headings, from the same sources the pages render. Run by `bun run gen` after
// generate-content.mjs (it reads generated.json). Never hand-edit the output.
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import generated from "../src/content/generated.json";
import {
	compatSections,
	contributeSections,
	flowSections,
	installSections,
	referenceSections,
	type Section,
} from "../src/content/sections";
import { enUS } from "../src/i18n";
import type { StoredEntry } from "../src/search";

const t = enUS;

// A script a skill is built around: named five times or more in its body.
function tools(body: string): string {
	const counts = new Map<string, number>();
	for (const m of body.matchAll(/\b([a-z][a-z0-9_-]*)\.(?:py|sh|ts|mjs)\b/g))
		counts.set(m[1] as string, (counts.get(m[1] as string) ?? 0) + 1);
	return [...counts]
		.filter(([, n]) => n >= 5)
		.map(([name]) => name)
		.join(" ");
}

// Skills carry only what is not already in generated.json (their scripts); the
// site fills in name, command, plugin, description and triggers from the same
// skill data at load (src/content.ts), so the bundle holds each description once.
const entries: StoredEntry[] = generated.skills.map((s) => ({
	kind: "skill",
	id: s.id,
	...(tools(s.body) ? { tools: tools(s.body) } : {}),
}));

function codeWords(sections: Section[]): string {
	return sections.flatMap((s) => (s.kind === "code" ? [s.code] : [])).join(" ");
}

function headingsOf(sections: Section[]): string[] {
	return sections.flatMap((s) => {
		if (s.kind === "heading") return [s.text];
		if (s.kind === "entries") return s.items.map((i) => i.term);
		if (s.kind === "table" && s.rowHeaders)
			return s.rows.map((r) => r[0] ?? "");
		return [];
	});
}

const pages: {
	id: string;
	url: string;
	title: string;
	lede: string;
	sections: Section[];
}[] = [
	{
		id: "install",
		url: "/use/install",
		title: t.install.title,
		lede: t.install.lede,
		sections: installSections(t),
	},
	{
		id: "compatibility",
		url: "/use/compatibility",
		title: t.compat.title,
		lede: t.compat.lede,
		sections: compatSections(t, generated.tiers as never),
	},
	{
		id: "flow",
		url: "/use/flow",
		title: t.flow.title,
		lede: t.flow.lede,
		sections: flowSections(t),
	},
	{
		id: "contribute",
		url: "/contribute",
		title: t.contribute.title,
		lede: t.contribute.lede,
		sections: contributeSections(t, generated.plugins),
	},
	{
		id: "reference",
		url: "/reference",
		title: t.reference.title,
		lede: t.reference.lede,
		sections: referenceSections(t),
	},
	{
		id: "changelog",
		url: "/changelog",
		title: t.changelog.title,
		lede: t.changelog.lede,
		sections: [],
	},
];

for (const p of pages) {
	const heads = headingsOf(p.sections);
	entries.push({
		kind: "page",
		id: p.id,
		title: p.title,
		text: `${p.lede} ${codeWords(p.sections)}`,
		headings: heads.join(" "),
		url: p.url,
	});
	heads.forEach((h, i) => {
		entries.push({
			kind: "heading",
			id: `${p.id}/${i}`,
			title: h,
			text: p.title,
			url: p.url,
		});
	});
}

const out = join(import.meta.dirname, "../src/content/search-index.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(entries)}\n`);
console.log(`wrote ${out}: ${entries.length} entries`);
