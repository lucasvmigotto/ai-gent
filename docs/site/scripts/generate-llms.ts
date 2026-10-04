// Emits the LLM-readable output from the same content the pages render:
// llms.txt, llms-full.txt and one .md per page and locale.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import generated from "../src/content/generated.json";
import { LOCALES, type Locale, messages } from "../src/i18n";

const site = fileURLToPath(new URL("..", import.meta.url));
const dist = join(site, "dist");
const base =
	process.env.VITE_SITE_URL || "https://docs.lucasvmigotto.me/ai-gent";

interface Page {
	slug: string;
	title: string;
	description: string;
	body: string;
	optional?: boolean;
}

function skillPages(locale: Locale): Page[] {
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
			`**${t.skill.invoke}:** \`/${s.plugin}:${s.skill}\` (Claude Code) · \`/${s.id}\` (opencode)`,
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

function staticPages(locale: Locale): Page[] {
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
				`## ${t.install.claude}`,
				"",
				"```sh",
				"curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh -s -- --target claude",
				"```",
				"",
				`## ${t.install.opencode}`,
				"",
				"```sh",
				"curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh -s -- --target opencode",
				"```",
				"",
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
				`## ${t.contribute.conventions}`,
				"",
				t.contribute.conventionsBody,
				"",
				`## ${t.contribute.checks}`,
				"",
				t.contribute.checksBody,
				"",
				"```sh",
				"./scripts/check.sh",
				"```",
				"",
				`## ${t.contribute.releases}`,
				"",
				t.contribute.releasesBody,
				"",
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
				`- ${t.reference.pipeline}: shared/pipeline.md`,
				`- ${t.reference.containers}: shared/containers.md`,
				`- ${t.reference.databases}: plugins/db/references/safety.md`,
				`- ${t.reference.guards}: ${t.reference.guardsBody}`,
				`- ${t.reference.readingCode}: shared/reading-code.md`,
				`- ${t.reference.specKit}: shared/spec-kit.md`,
				"",
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

function llmsTxt(locale: Locale, pages: Page[]): string {
	const t = messages[locale];
	const lines = [
		`# ai-gent docs`,
		"",
		`> ${t.meta.description}`,
		"",
		`ai-gent is ${generated.counts.plugins} plugins and ${generated.counts.skills} skills for Claude Code and opencode. Current version ${generated.version}.`,
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

for (const locale of LOCALES) {
	const pages = [...staticPages(locale), ...skillPages(locale)];
	const dir = join(dist, "docs", locale);
	mkdirSync(dir, { recursive: true });
	for (const p of pages) {
		const file = join(dir, `${p.slug}.md`);
		mkdirSync(join(file, ".."), { recursive: true });
		writeFileSync(file, `${p.body.trim()}\n`);
	}
	writeFileSync(join(dist, `llms-${locale}.txt`), llmsTxt(locale, pages));
	writeFileSync(
		join(dist, `llms-full-${locale}.txt`),
		`${pages.map((p) => p.body.trim()).join("\n\n---\n\n")}\n`,
	);
}

// llms.txt / llms-full.txt default to the primary locale (en-US).
writeFileSync(
	join(dist, "llms.txt"),
	llmsTxt("en-US", [...staticPages("en-US"), ...skillPages("en-US")]),
);
writeFileSync(
	join(dist, "llms-full.txt"),
	`${[...staticPages("en-US"), ...skillPages("en-US")]
		.map((p) => p.body.trim())
		.join("\n\n---\n\n")}\n`,
);

console.log(
	`wrote llms.txt, llms-full.txt and per-page Markdown for ${LOCALES.length} locale(s)`,
);
