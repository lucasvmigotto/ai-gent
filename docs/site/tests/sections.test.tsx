import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SectionBlocks } from "../src/components";
import {
	contributeSections,
	installSections,
	referenceSections,
	type Section,
	sectionsToMarkdown,
} from "../src/content/sections";
import { enUS, I18nProvider } from "../src/i18n";

// GH #77: install/contribute/reference used to be written twice — once as
// JSX in src/pages.tsx, once as a hand-built line list in
// scripts/llms-pages.ts — so a section added to one could be forgotten in
// the other. Now both read the same Section[] from src/content/sections.ts.
// These tests exercise that one source directly, rather than the page
// components, to show the two renderings can no longer drift apart.

const demoPlugins = [{ name: "demo-plugin", version: "1.2.3" }];

const BUILDERS: Record<string, () => Section[]> = {
	install: () => installSections(enUS),
	contribute: () => contributeSections(enUS, demoPlugins),
	reference: () => referenceSections(enUS),
};

function renderSections(sections: Section[]) {
	return render(
		<I18nProvider>
			<SectionBlocks sections={sections} />
		</I18nProvider>,
	);
}

describe("shared page sections (src/content/sections.ts)", () => {
	function headingTextsOf(sections: Section[]): string[] {
		return sections.filter((s) => s.kind === "heading").map((s) => s.text);
	}
	function termsOf(sections: Section[]): string[] {
		return sections
			.filter((s) => s.kind === "entries" || s.kind === "list")
			.flatMap((s) => s.items.map((item) => item.term));
	}

	for (const [name, build] of Object.entries(BUILDERS)) {
		it(`${name}: every heading and term is in the Markdown render`, () => {
			const sections = build();
			const headingTexts = headingTextsOf(sections);
			const terms = termsOf(sections);
			expect(headingTexts.length + terms.length).toBeGreaterThan(0);

			const md = sectionsToMarkdown(sections).join("\n");
			for (const h of [...headingTexts, ...terms]) expect(md).toContain(h);
		});

		it(`${name}: every heading and term is in the HTML render`, () => {
			const sections = build();
			const headingTexts = headingTextsOf(sections);
			const terms = termsOf(sections);

			const { container, unmount } = renderSections(sections);
			const domHeadings = [...container.querySelectorAll("h2, h3")].map((h) =>
				(h.textContent ?? "").trim(),
			);
			const domTerms = [...container.querySelectorAll("dt")].map((d) =>
				(d.textContent ?? "").trim(),
			);
			unmount();

			for (const h of headingTexts) expect(domHeadings).toContain(h);
			for (const term of terms) expect(domTerms).toContain(term);
		});
	}

	// Proves the detection itself works: a section present in the full
	// array but missing from a (deliberately truncated) copy shows up as
	// absent from that copy's Markdown — this is exactly the "one page
	// gains a section the other lacks" bug #77 described. Then the "fix":
	// rendering from the same full array leaves nothing missing.
	it("a section dropped from the shared array is missing from its Markdown twin — and found once restored", () => {
		const full = installSections(enUS);
		expect(full.length).toBeGreaterThan(1);

		const droppedLastHeading = full.slice(0, -1);
		const lastSection = full.at(-1);
		expect(lastSection).toBeDefined();
		let lastText: string | undefined;
		if (lastSection?.kind === "heading") lastText = lastSection.text;
		else if (lastSection?.kind === "entries")
			lastText = lastSection.items[0]?.term;
		expect(lastText).toBeTruthy();

		const brokenMarkdown = sectionsToMarkdown(droppedLastHeading).join("\n");
		expect(brokenMarkdown).not.toContain(lastText as string);

		const fixedMarkdown = sectionsToMarkdown(full).join("\n");
		expect(fixedMarkdown).toContain(lastText as string);
	});
});
