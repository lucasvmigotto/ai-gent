// The LLM-readable pages, built from the same content the pages render. Pure
// functions: generate-llms.ts writes them, tests/llms.test.tsx checks them.
import generated from "../src/content/generated.json";
import {
	contributeSections,
	flowSections,
	installSections,
	referenceSections,
	type Section,
	sectionsToMarkdown,
} from "../src/content/sections";
import { type Locale, type Messages, messages } from "../src/i18n";

export interface Page {
	slug: string;
	title: string;
	description: string;
	body: string;
	optional?: boolean;
}

export function skillPages(locale: Locale): Page[] {
	const t = messages[locale];
	return generated.skills.map((s) => ({
		slug: `skills/${s.id}`,
		title: s.name,
		description: s.description.split(". ")[0] ?? s.description,
		body: [
			`# ${s.name}`,
			"",
			s.description,
			"",
			`**${t.skill.invoke}:** \`/${s.plugin}:${s.skill}\` (Claude Code) · \`${s.id}\` (opencode, Codex, Cursor and the other agents)`,
			"",
			`**${t.skill.generated(s.plugin, pluginVersion(s.plugin), generated.version)}**`,
			"",
			t.skill.bodyNote,
			"",
			s.body,
			"",
			`[${t.skill.source}](${s.sourceUrl})`,
			"",
		].join("\n"),
	}));
}

function pluginVersion(plugin: string): string {
	return generated.plugins.find((p) => p.name === plugin)?.version ?? "?";
}

// flow's step loop and support grid are markdown-only here (no Section
// variant fits their icons-and-tooltips JSX); flowSections(t) still decides
// where they fall relative to the headings, so both renderers share one
// sequence even though the body of these two blocks is written once, here.
function flowCustomMarkdown(id: string, t: Messages): string[] {
	switch (id) {
		case "loop":
			return [
				...t.flow.steps.flatMap((s, i) => [
					`${i + 1}. **${s.title}**${s.asks ? ` (${t.flow.asks})` : ""} — ${s.skills.join(", ")}. ${s.body}`,
					...(s.track ?? []).map(
						(x) => `   - ${x.title} — ${x.skills.join(", ")}. ${x.body}`,
					),
					...(s.loop ? [`   - ${t.flow.whenRed}: ${s.loop}`] : []),
				]),
				"",
			];
		case "support":
			return [
				...t.flow.support.map((x) => `- ${x.skills.join(", ")}: ${x.body}`),
				"",
				t.flow.guards,
				"",
			];
		default:
			return [];
	}
}

export function staticPages(locale: Locale): Page[] {
	const t = messages[locale];
	return [
		{
			slug: "install",
			title: t.install.title,
			description: t.install.lede,
			body: [
				`# ${t.install.title}`,
				"",
				t.install.lede,
				"",
				...sectionsToMarkdown(installSections(t)),
			].join("\n"),
		},
		{
			slug: "flow",
			title: t.flow.title,
			description: t.flow.lede,
			body: [
				`# ${t.flow.title}`,
				"",
				t.flow.lede,
				"",
				...flowSections(t).flatMap((s: Section) =>
					s.kind === "custom"
						? flowCustomMarkdown(s.id, t)
						: sectionsToMarkdown([s]),
				),
			].join("\n"),
		},
		{
			slug: "contribute",
			title: t.contribute.title,
			description: t.contribute.lede,
			body: [
				`# ${t.contribute.title}`,
				"",
				t.contribute.lede,
				"",
				...sectionsToMarkdown(contributeSections(t, generated.plugins)),
			].join("\n"),
		},
		{
			slug: "reference",
			title: t.reference.title,
			description: t.reference.lede,
			body: [
				`# ${t.reference.title}`,
				"",
				t.reference.lede,
				"",
				...sectionsToMarkdown(referenceSections(t)),
			].join("\n"),
		},
		{
			slug: "changelog",
			title: t.changelog.title,
			description: t.changelog.lede,
			body: generated.changelog,
			optional: true,
		},
	];
}

export function llmsTxt(locale: Locale, pages: Page[], base: string): string {
	const t = messages[locale];
	const lines = [
		`# ai-gent docs`,
		"",
		`> ${t.meta.description}`,
		"",
		`ai-gent is ${generated.counts.plugins} plugins and ${generated.counts.skills} skills for Claude Code and other coding agents. Current version ${generated.version}.`,
		"",
		"## Use",
		"",
	];
	const use = pages.filter(
		(p) => p.slug === "install" || p.slug.startsWith("skills/"),
	);
	const ref = pages.filter((p) => !use.includes(p) && !p.optional);
	const opt = pages.filter((p) => p.optional);
	for (const p of use)
		lines.push(
			`- [${p.title}](${base}/docs/${locale}/${p.slug}.md): ${p.description}`,
		);
	lines.push("", "## Reference", "");
	for (const p of ref)
		lines.push(
			`- [${p.title}](${base}/docs/${locale}/${p.slug}.md): ${p.description}`,
		);
	if (opt.length) {
		lines.push("", "## Optional", "");
		for (const p of opt)
			lines.push(
				`- [${p.title}](${base}/docs/${locale}/${p.slug}.md): ${p.description}`,
			);
	}
	lines.push("");
	return lines.join("\n");
}
