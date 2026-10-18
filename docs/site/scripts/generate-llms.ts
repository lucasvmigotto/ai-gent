// Emits the LLM-readable output: llms.txt, llms-full.txt and one .md per
// page and locale (the pages themselves are built in llms-pages.ts).
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { LOCALES } from "../src/i18n";
import { llmsTxt, skillPages, staticPages } from "./llms-pages";

const site = fileURLToPath(new URL("..", import.meta.url));
const dist = join(site, "dist");
const base =
	process.env.VITE_SITE_URL || "https://docs.lucasvmigotto.me/ai-gent";

for (const locale of LOCALES) {
	const pages = [...staticPages(locale), ...skillPages(locale)];
	const dir = join(dist, "docs", locale);
	mkdirSync(dir, { recursive: true });
	for (const p of pages) {
		const file = join(dir, `${p.slug}.md`);
		mkdirSync(join(file, ".."), { recursive: true });
		writeFileSync(file, `${p.body.trim()}\n`);
	}
	writeFileSync(join(dist, `llms-${locale}.txt`), llmsTxt(locale, pages, base));
	writeFileSync(
		join(dist, `llms-full-${locale}.txt`),
		`${pages.map((p) => p.body.trim()).join("\n\n---\n\n")}\n`,
	);
}

// llms.txt / llms-full.txt default to the primary locale (en-US).
writeFileSync(
	join(dist, "llms.txt"),
	llmsTxt("en-US", [...staticPages("en-US"), ...skillPages("en-US")], base),
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
