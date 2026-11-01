// The one typed description of what is on a static page, past its <h1> and
// lede. Both the page component (../pages.tsx, via <SectionBlocks>) and the
// LLM Markdown output (../../scripts/llms-pages.ts, via sectionsToMarkdown)
// render these same Section[] arrays, so there is exactly one place that
// names a page's structure — fixes https://github.com/lucasvmigotto/ai-gent/issues/77.
//
// "entries" and "list" both describe term/body pairs, rendered as a <dl> in
// either case; they differ in how much each item is worth on the page:
// "entries" promotes each term to its own Markdown heading (install's
// troubleshooting, long enough to navigate to), "list" keeps every item a
// single bullet (reference's short index). A "custom" section names a block
// whose content is itself data-driven (e.g. flow's step list) and therefore
// already shared without needing a Section — the id just keeps it in the
// same ordered sequence as the rest of the page for both renderers.
import type { Messages } from "../i18n";

export interface DlItem {
	term: string;
	body?: string;
	code?: string;
	/** Repository-relative path: a link in HTML, plain text in Markdown. */
	path?: string;
}

export type Section =
	| { kind: "heading"; level: 2 | 3; text: string }
	| { kind: "paragraph"; text: string }
	| { kind: "code"; code: string; label?: string }
	| { kind: "entries"; items: DlItem[] }
	| { kind: "list"; items: DlItem[] }
	| { kind: "table"; headers: string[]; rows: string[][] }
	| { kind: "custom"; id: string };

const INSTALL_CMD =
	"curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh";
const INSTALL_CMD_TARGETS =
	"curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh -s -- --target claude,opencode,codex,cursor";
const VERIFY_CMD =
	"claude plugin details project@skills-dir\nopencode plugin list";

export function installSections(t: Messages): Section[] {
	return [
		{ kind: "heading", level: 2, text: t.install.choose },
		{ kind: "heading", level: 3, text: t.install.claude },
		{ kind: "code", code: INSTALL_CMD },
		{ kind: "paragraph", text: t.install.claudeBody },
		{ kind: "heading", level: 3, text: t.install.others },
		{ kind: "code", code: INSTALL_CMD_TARGETS },
		{ kind: "paragraph", text: t.install.othersBody },
		{ kind: "heading", level: 2, text: t.install.verify },
		{ kind: "code", code: VERIFY_CMD },
		{ kind: "heading", level: 2, text: t.install.updates },
		{ kind: "paragraph", text: t.install.updatesBody },
		{ kind: "heading", level: 2, text: t.install.firstSkill },
		{ kind: "paragraph", text: t.install.firstSkillBody },
		{ kind: "heading", level: 2, text: t.install.troubleshooting },
		{
			kind: "entries",
			items: t.install.troubleshootingItems.map((item) => ({
				term: item.problem,
				body: item.fix,
				code: item.command,
			})),
		},
	];
}

export function contributeSections(
	t: Messages,
	plugins: { name: string; version: string }[],
): Section[] {
	return [
		{ kind: "heading", level: 2, text: t.contribute.conventions },
		{ kind: "paragraph", text: t.contribute.conventionsBody },
		{ kind: "heading", level: 2, text: t.contribute.checks },
		{ kind: "paragraph", text: t.contribute.checksBody },
		{ kind: "code", code: "./scripts/check.sh" },
		{ kind: "heading", level: 2, text: t.contribute.layout },
		{ kind: "paragraph", text: t.contribute.layoutBody },
		{ kind: "heading", level: 2, text: t.contribute.releases },
		{ kind: "paragraph", text: t.contribute.releasesBody },
		{ kind: "heading", level: 2, text: t.contribute.versions },
		{
			kind: "table",
			headers: ["plugin", "version"],
			rows: plugins.map((p) => [p.name, p.version]),
		},
	];
}

export function referenceSections(t: Messages): Section[] {
	return [
		{
			kind: "list",
			items: [
				{ term: t.reference.pipeline, path: "shared/pipeline.md" },
				{ term: t.reference.containers, path: "shared/containers.md" },
				{ term: t.reference.guards, body: t.reference.guardsBody },
				{ term: t.reference.mods, body: t.reference.modsBody },
				{
					term: t.reference.databases,
					path: "plugins/db/references/safety.md",
				},
				{ term: t.reference.readingCode, path: "shared/reading-code.md" },
				{ term: t.reference.specKit, path: "shared/spec-kit.md" },
				{ term: t.reference.secrets, path: "shared/secrets.md" },
				{ term: t.reference.agentRoles, path: "shared/agent-roles.md" },
				{
					term: t.reference.trackerSync,
					path: "plugins/repo/references/sync.md",
				},
			],
		},
	];
}

// flow's step loop and support grid are already single-sourced from
// t.flow.steps / t.flow.support (both renderers map over the same array);
// what used to drift was the surrounding skeleton — which headings exist,
// in which order. "custom" sections keep that skeleton in one list: each
// renderer fills "loop" and "support" from the shared arrays, in whichever
// shape (JSX with icons, or Markdown) suits it.
export function flowSections(t: Messages): Section[] {
	return [
		{ kind: "custom", id: "glance" },
		{ kind: "heading", level: 2, text: t.flow.loop },
		{ kind: "custom", id: "loop" },
		{ kind: "heading", level: 2, text: t.flow.autonomyTitle },
		{ kind: "paragraph", text: t.flow.autonomy },
		{ kind: "heading", level: 2, text: t.flow.supportTitle },
		{ kind: "custom", id: "support" },
	];
}

export function sectionsToMarkdown(sections: Section[]): string[] {
	const lines: string[] = [];
	for (const s of sections) {
		switch (s.kind) {
			case "heading":
				lines.push(`${"#".repeat(s.level)} ${s.text}`, "");
				break;
			case "paragraph":
				lines.push(s.text, "");
				break;
			case "code":
				lines.push("```sh", s.code, "```", "");
				break;
			case "entries":
				for (const item of s.items) {
					lines.push(`### ${item.term}`, "");
					if (item.body) lines.push(item.body, "");
					if (item.code) lines.push("```sh", item.code, "```", "");
				}
				break;
			case "list":
				for (const item of s.items)
					lines.push(`- ${item.term}: ${item.path ?? item.body ?? ""}`);
				lines.push("");
				break;
			case "table":
				lines.push(`| ${s.headers.join(" | ")} |`);
				lines.push(`|${s.headers.map(() => "---").join("|")}|`);
				for (const row of s.rows) lines.push(`| ${row.join(" | ")} |`);
				lines.push("");
				break;
			case "custom":
				// Filled in by the caller (flow's steps/support come from shared
				// arrays, not from this module) — nothing to emit here.
				break;
		}
	}
	return lines;
}

/** A stable, content-derived React key for a Section — never the array
 *  index, which breaks when a page's own list of sections changes. */
export function sectionKey(s: Section): string {
	switch (s.kind) {
		case "heading":
		case "paragraph":
			return `${s.kind}:${s.text}`;
		case "code":
			return `code:${s.code}`;
		case "entries":
		case "list":
			return `${s.kind}:${s.items.map((item) => item.term).join(",")}`;
		case "table":
			return `table:${s.headers.join(",")}`;
		case "custom":
			return `custom:${s.id}`;
	}
}

/** Every heading a Section[] will produce, in order — used to check the
 *  rendered page and the generated Markdown stay in step (tests/llms.test.tsx). */
export function sectionHeadings(sections: Section[]): string[] {
	const found: string[] = [];
	for (const s of sections) {
		if (s.kind === "heading") found.push(s.text);
		if (s.kind === "entries") for (const item of s.items) found.push(item.term);
	}
	return found;
}
