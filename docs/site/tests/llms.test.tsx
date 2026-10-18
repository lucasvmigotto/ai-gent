import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { llmsTxt, skillPages, staticPages } from "../scripts/llms-pages";
import { enUS, I18nProvider, LOCALES } from "../src/i18n";
import {
	ChangelogPage,
	ContributePage,
	FlowPage,
	InstallPage,
	ReferencePage,
} from "../src/pages";

// Each static page and its Markdown twin come from the same messages but are
// written twice (JSX and a line list); this fails when one gains a section the
// other lacks.
const PAGES = {
	install: InstallPage,
	flow: FlowPage,
	contribute: ContributePage,
	reference: ReferencePage,
} as const;

function htmlHeadings(Page: () => React.ReactNode): string[] {
	const { container, unmount } = render(
		<I18nProvider>
			<MemoryRouter>
				<Page />
			</MemoryRouter>
		</I18nProvider>,
	);
	// Screen-reader-only headings label decorative strips; flow steps carry a
	// number and an "asks you" badge around the title.
	const found = [...container.querySelectorAll("h2:not(.sr-only), h3")].map(
		(h) =>
			(h.textContent ?? "")
				.replace(/^\d+/, "")
				.replace(enUS.flow.asks, "")
				.trim(),
	);
	unmount();
	return found;
}

function markdownHeadings(body: string): string[] {
	let fenced = false;
	const found: string[] = [];
	for (const line of body.split("\n")) {
		if (line.startsWith("```")) fenced = !fenced;
		const m = fenced ? null : /^#{2,3} (.+)$/.exec(line);
		if (m?.[1]) found.push(m[1].trim());
	}
	return found;
}

describe("LLM output", () => {
	for (const [slug, Page] of Object.entries(PAGES)) {
		it(`${slug}: every section of the page is in its Markdown`, () => {
			const body =
				staticPages("en-US").find((p) => p.slug === slug)?.body ?? "";
			const md = markdownHeadings(body);
			const missing = htmlHeadings(Page).filter(
				(h) => !md.includes(h) && !body.includes(`**${h}**`),
			);
			expect(missing).toEqual([]);
		});
	}

	it("has a page for every static route, in every locale", () => {
		for (const locale of LOCALES) {
			const slugs = staticPages(locale).map((p) => p.slug);
			expect(slugs).toEqual(
				expect.arrayContaining([...Object.keys(PAGES), "changelog"]),
			);
		}
		expect(ChangelogPage).toBeTypeOf("function");
	});

	it("has one Markdown page per skill, each linking its source", () => {
		const pages = skillPages("en-US");
		expect(pages.length).toBeGreaterThan(0);
		for (const p of pages) {
			expect(p.body).toMatch(/\]\(https:\/\/github\.com\//);
		}
	});

	it("lists every page in llms.txt", () => {
		const pages = [...staticPages("en-US"), ...skillPages("en-US")];
		const index = llmsTxt("en-US", pages, "https://example.test/ai-gent");
		for (const p of pages) {
			expect(index).toContain(`/docs/en-US/${p.slug}.md`);
		}
	});
});
