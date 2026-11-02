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

import type { TierMatrix } from "../content";
import type { Messages } from "../i18n";
import { installFacts } from "./install-facts";

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
	| {
			kind: "table";
			headers: string[];
			rows: string[][];
			/** Makes the first cell of each row a row header (th scope=row) and
			 *  names the table as a scrollable region. */
			rowHeaders?: boolean;
			label?: string;
	  }
	| { kind: "custom"; id: string };

const F = installFacts;

export function installSections(t: Messages): Section[] {
	const i = t.install;
	return [
		{ kind: "heading", level: 2, text: i.choose },
		{ kind: "paragraph", text: i.chooseBody },
		{ kind: "heading", level: 3, text: i.claude },
		{ kind: "code", code: F.install.command },
		{ kind: "paragraph", text: i.claudeBody },
		{ kind: "heading", level: 3, text: i.others },
		{ kind: "code", code: F.targets.command },
		{ kind: "paragraph", text: i.othersBody },
		{ kind: "paragraph", text: i.inspect },
		{ kind: "code", code: F.inspect.command },
		{ kind: "heading", level: 2, text: i.os },
		{ kind: "heading", level: 3, text: i.osUnix },
		{ kind: "paragraph", text: i.osUnixBody },
		{ kind: "heading", level: 3, text: i.osWindows },
		{ kind: "paragraph", text: i.osWindowsBody },
		{ kind: "heading", level: 2, text: i.pin },
		{ kind: "paragraph", text: i.pinBody },
		{ kind: "code", code: F.pin.command },
		{ kind: "paragraph", text: i.unpinBody },
		{ kind: "code", code: F.unpin.command },
		{ kind: "heading", level: 2, text: i.verify },
		{ kind: "paragraph", text: i.verifyBody },
		{ kind: "paragraph", text: i.attestationBody },
		{ kind: "code", code: F.verifyAttestation.command },
		{ kind: "paragraph", text: i.attestationManual },
		{ kind: "code", code: F.verifyManual.command },
		{ kind: "paragraph", text: i.verifyInstalled },
		{ kind: "code", code: F.verifyClaude.command },
		{ kind: "heading", level: 2, text: i.updates },
		{ kind: "paragraph", text: i.updatesBody },
		{ kind: "paragraph", text: i.rollbackBody },
		{
			kind: "code",
			code: [F.list, F.rollback, F.prune].map((f) => f.command).join("\n"),
		},
		{ kind: "heading", level: 2, text: i.afterInstall },
		{ kind: "paragraph", text: i.afterInstallBody },
		{
			kind: "code",
			code: [F.doctor, F.updateCheck, F.update, F.updateRollback]
				.map((f) => f.command)
				.join("\n"),
		},
		{ kind: "heading", level: 2, text: i.migrate },
		{ kind: "paragraph", text: i.migrateBody },
		{ kind: "code", code: F.migrate.command },
		{ kind: "heading", level: 2, text: i.firstSkill },
		{ kind: "paragraph", text: i.firstSkillBody },
		{ kind: "heading", level: 2, text: i.troubleshooting },
		{
			kind: "entries",
			items: i.troubleshootingItems.map((item) => ({
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

export function compatSections(t: Messages, matrix: TierMatrix): Section[] {
	const c = t.compat;
	return [
		{ kind: "heading", level: 2, text: c.matrix },
		{ kind: "paragraph", text: c.legend },
		{
			kind: "table",
			rowHeaders: true,
			label: c.matrixLabel,
			headers: [
				c.feature,
				...matrix.tiers.map(
					(tier) => `${tier.label} (${tier.agents.join(", ")})`,
				),
			],
			rows: matrix.rows.map((row) => [
				row.feature,
				...matrix.tiers.map((tier) => {
					const cell = row.cells[tier.id];
					if (!cell) return "";
					const s = c.states[cell.state];
					return `${s.symbol} ${s.label}: ${cell.reason}`;
				}),
			]),
		},
		{ kind: "heading", level: 2, text: c.tiers },
		{ kind: "paragraph", text: c.tiersBody },
		{
			kind: "list",
			items: matrix.tiers.map((tier) => ({
				term: `${tier.label}: ${tier.agents.join(", ")}`,
				body: tier.policy,
			})),
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
