import {
	createContext,
	type ReactNode,
	useContext,
	useEffect,
	useState,
} from "react";

export const LOCALES = ["en-US"] as const;
export type Locale = (typeof LOCALES)[number];

export interface FlowStep {
	key: string;
	title: string;
	skills: string[];
	body: string;
	asks?: boolean;
	track?: { title: string; skills: string[]; body: string }[];
	loop?: string;
}

export interface Messages {
	meta: { title: string; description: string };
	a11y: { skip: string; external: string };
	nav: {
		use: string;
		skills: string;
		contribute: string;
		reference: string;
		changelog: string;
		flow: string;
		install: string;
		search: string;
		searchLabel: string;
		language: string;
		menu: string;
		close: string;
		home: string;
	};
	landing: {
		title: string;
		lede: string;
		placeholder: string;
		counts: (plugins: number, skills: number) => string;
		why: string;
		installCta: string;
		flowCta: string;
		index: string;
	};
	search: {
		emptyTitle: (query: string) => string;
		emptyBody: string;
		results: (count: number) => string;
		clearing: string;
	};
	skills: { title: string; lede: string; filter: string; all: string };
	skill: {
		when: string;
		copy: string;
		copied: string;
		generated: (plugin: string, version: string, site: string) => string;
		source: string;
		related: string;
		back: string;
		bodyNote: string;
		invoke: string;
	};
	install: {
		title: string;
		lede: string;
		choose: string;
		claude: string;
		claudeBody: string;
		others: string;
		othersBody: string;
		verify: string;
		updates: string;
		updatesBody: string;
		firstSkill: string;
		firstSkillBody: string;
		troubleshooting: string;
		next: string;
	};
	contribute: {
		title: string;
		lede: string;
		conventions: string;
		conventionsBody: string;
		checks: string;
		checksBody: string;
		releases: string;
		releasesBody: string;
		versions: string;
		layout: string;
		layoutBody: string;
	};
	reference: {
		title: string;
		lede: string;
		pipeline: string;
		containers: string;
		databases: string;
		guards: string;
		readingCode: string;
		specKit: string;
		secrets: string;
		agentRoles: string;
		trackerSync: string;
		guardsBody: string;
	};
	changelog: { title: string; lede: string };
	flow: {
		title: string;
		lede: string;
		glance: string;
		loop: string;
		asks: string;
		inEachTrack: string;
		whenRed: string;
		steps: FlowStep[];
		autonomyTitle: string;
		autonomy: string;
		supportTitle: string;
		support: { skills: string[]; body: string }[];
		guards: string;
	};
	notFound: { title: string; body: string };
	footer: {
		license: string;
		licenseTitle: string;
		version: string;
		theme: string;
		light: string;
		dark: string;
	};
}

export const enUS: Messages = {
	meta: {
		title:
			"ai-gent docs — skills and plugins for Claude Code and other coding agents",
		description:
			"Install ai-gent, find the right skill for the task, and contribute.",
	},
	a11y: { skip: "Skip to content", external: "opens in a new tab" },
	nav: {
		use: "Use",
		skills: "Skills",
		contribute: "Contribute",
		reference: "Reference",
		changelog: "Changelog",
		flow: "Flow",
		install: "Install",
		search: "Search skills",
		searchLabel: "Search skills",
		language: "Language",
		menu: "Menu",
		close: "Close menu",
		home: "ai-gent docs home",
	},
	landing: {
		title: "Find the skill for the task.",
		lede: "ai-gent is a set of plugins and skills for Claude Code and other coding agents. Type what you are trying to do.",
		placeholder: "What are you trying to do?",
		counts: (plugins, skills) =>
			`${skills} skills · ${plugins} plugins · ${LOCALES.join(" + ")}`,
		why: "Free and open source, made to take the busywork off developers' days — and frugal with tokens: one-line skill descriptions, details read only when a task needs them, CI watched by a small model, code read in a compact form.",
		installCta: "Install ai-gent",
		flowCta: "See the autonomous flow",
		index: "All skills, by plugin",
	},
	search: {
		emptyTitle: (query) => `No skill matches “${query}”.`,
		emptyBody: "Try fewer words, or browse the full index.",
		results: (count) => `${count} skills match`,
		clearing: "Search cleared",
	},
	skills: {
		title: "Skills",
		lede: "Every skill, grouped by plugin.",
		filter: "Filter by plugin",
		all: "All plugins",
	},
	skill: {
		when: "When to invoke",
		copy: "Copy",
		copied: "Copied",
		generated: (plugin, version, site) =>
			`Generated from ${plugin} v${version} · ai-gent ${site}`,
		source: "Read the source SKILL.md",
		related: "Related skills",
		back: "All skills",
		bodyNote:
			"The instructions below are the skill's source text, kept in English.",
		invoke: "Invoke",
	},
	install: {
		title: "Install ai-gent",
		lede: "One command. On a terminal it asks which agents to install for, with Claude Code preselected. Re-run it to update.",
		choose: "Choose your agents",
		claude: "Claude Code (default)",
		claudeBody:
			"Without a terminal, or with --skip-prompt, it installs your previous choice, or Claude Code alone the first time.",
		others: "Other agents",
		othersBody:
			"opencode, Codex, Cursor, Gemini CLI, GitHub Copilot, Amp, Kiro and Windsurf are opt-in: pick them in the selector or name them with --target. Codex, Cursor, Gemini CLI, Copilot and Amp share one ~/.agents/skills directory. Only Claude Code and opencode enforce the guards (git, db, secrets, repo); elsewhere they are instructions.",
		verify: "Verify",
		updates: "Updates",
		updatesBody:
			"When a Claude Code session starts, a SessionStart check prints one line if a newer release is known — the check reads a weekly cache and never blocks startup. Pinned checkouts stay silent, and AI_GENT_NO_UPDATE_CHECK=1 disables it. The session that notices an update only proposes it; applying it takes effect next session.",
		firstSkill: "Invoke your first skill",
		firstSkillBody:
			"Type what you want in plain words and the matching skill loads, or invoke one directly.",
		troubleshooting: "If it does not work",
		next: "Next: browse the skills",
	},
	contribute: {
		title: "Contribute",
		lede: "Change a skill or plugin and land it green.",
		conventions: "Conventions",
		conventionsBody:
			"Prose changes follow Conventional Commits; branch per context; descriptions are single-line, at most 400 characters, no “: ” or “ #”.",
		checks: "Run the checks",
		checksBody:
			"scripts/check.sh validates metadata, cross-references, symlinks, shell scripts and the test suites. CI runs the same script.",
		releases: "Releases",
		releasesBody:
			"Automated: pushes to main decide the bump from the commits, bump the changed plugins, promote the changelog and tag.",
		versions: "Versions",
		layout: "Layout",
		layoutBody:
			"A plugin is plugins/<name>/ with a manifest and skills/<skill>/SKILL.md. OpenCode-native code lives in opencode/.",
	},
	reference: {
		title: "Reference",
		lede: "The contracts and rules the toolkit runs on.",
		pipeline: "Product pipeline",
		containers: "Containers",
		databases: "Databases",
		guards: "Guard hooks",
		readingCode: "Reading code",
		specKit: "Spec Kit",
		secrets: "Secrets in pass",
		agentRoles: "Agent roles and models",
		trackerSync: "Specs on the tracker",
		guardsBody:
			"Four PreToolUse hooks enforce the git, db, secrets and repo rules in Claude Code; an OpenCode V2 plugin enforces the same rules there. The git plugin also runs a SessionStart check that prints one line when a newer ai-gent release is known.",
	},
	changelog: { title: "Changelog", lede: "Releases and what changed." },
	flow: {
		title: "The autonomous flow",
		lede: "From an idea to a released, documented product: what runs on its own, what asks you, and the skills that do each part.",
		glance: "At a glance",
		loop: "The loop, step by step",
		asks: "asks you",
		inEachTrack: "In each track",
		whenRed: "When CI is red",
		steps: [
			{
				key: "start",
				title: "Start",
				skills: ["project-init"],
				body: "A product brief from your idea or references. A new repository gets a one-line README as its initial commit and a dev branch; the files you handed in stay uncommitted.",
			},
			{
				key: "design",
				title: "Design",
				skills: ["project-architecture", "backend-domain", "frontend-uiux"],
				body: "Architecture and stack, the domain model and the UX vision — each a document you accept before the next stage builds on it.",
			},
			{
				key: "specify",
				title: "Specify",
				skills: [
					"project-spec",
					"frontend-spec",
					"backend-spec",
					"qa-strategy",
				],
				body: "Spec Kit features with stories and tasks, the OpenAPI contract between frontend and backend, and the test strategy.",
			},
			{
				key: "track",
				title: "Track",
				skills: ["repo-sync"],
				body: "An issue, work item or epic per feature and a child per story — GitHub, Azure Boards, GitLab, Jira or Bitbucket — created in one confirmed batch.",
			},
			{
				key: "develop",
				title: "Develop in parallel",
				skills: ["agents-parallel"],
				body: "Independent features, or a feature's frontend, backend and QA tracks, each on its own branch in a git worktree, with its own containers and ports.",
				track: [
					{
						title: "Plan and build with the right model",
						skills: ["agents-orchestrate", "compact-code"],
						body: "A strong model plans and reviews, a fast one builds, a small one searches; long code is read in a compact form.",
					},
					{
						title: "Implement, tests first",
						skills: ["frontend-build", "backend-build", "frontend-tui"],
						body: "Phase by phase, converged before Implemented; each commit references its story (Refs #121).",
					},
					{
						title: "Run it like CI",
						skills: ["devcontainer-workflow"],
						body: "Tests and builds run in the project's containers, the same ones CI uses.",
					},
				],
			},
			{
				key: "land",
				title: "Land",
				skills: ["git-workflow"],
				body: "Branches merge into dev one at a time, in dependency order, with the checks run after each.",
				asks: true,
			},
			{
				key: "watch",
				title: "Watch CI",
				skills: ["devsecops-watch"],
				body: "Every push is followed to the end by a small model in its own context, which reports back.",
				loop: "Red because of the change: the build stage fixes it on the same branch and pushes again — at most three rounds, then it stops and reports.",
			},
			{
				key: "verify",
				title: "Verify",
				skills: ["qa-e2e", "qa-load"],
				body: "Cross-stack journeys, and load tests where there are targets, pass in CI: the feature is Verified.",
			},
			{
				key: "close",
				title: "Close",
				skills: ["repo-sync"],
				body: "The feature's items close once its work is on the integration branch; the promotion lists Closes lines as a backstop.",
			},
			{
				key: "release",
				title: "Release",
				skills: ["git-workflow", "devsecops-pipeline"],
				body: "dev is promoted to main; the release workflow picks the version from the Conventional Commits, writes the changelog, tags and publishes.",
				asks: true,
			},
			{
				key: "document",
				title: "Document",
				skills: ["project-docs"],
				body: "The documentation site and llms.txt regenerate from the code and the specs, and deploy with the release.",
			},
		],
		autonomyTitle: "How autonomous",
		autonomy:
			"Questions come first: the stages that shape the product ask theirs before writing — five at a time, as choices with a recommendation, in as many rounds as it takes; whatever you decline to answer, the agent decides and records. Then give permission up front — “build it end to end, you can push” — and the loop runs without stopping: commits, pushes of work branches and dev, tracker batches and CI fixes. Promoting to main and releasing still ask unless that permission names them; force pushes and deletions always do. The guard hooks ask on every push, so a hands-off run also needs pushes allowed in your agent's permission settings.",
		supportTitle: "Alongside every step",
		support: [
			{
				skills: ["secrets-pass"],
				body: "Tokens and database passwords in pass, read only by the runner that needs them — never shown to the agent.",
			},
			{
				skills: ["db-connect", "db-inspect", "db-investigate"],
				body: "Databases through one audited runner: masked results, writes only on proven local containers.",
			},
			{
				skills: ["devcontainer-setup", "devcontainer-infra"],
				body: "Container-first environments and simulated infrastructure.",
			},
			{
				skills: ["devsecops-supply-chain", "devsecops-audit"],
				body: "Pinned dependencies, scanning, SBOMs and pipeline reviews.",
			},
			{
				skills: ["repo-issues", "repo-pr"],
				body: "Issues, work items and pull requests on any of the platforms.",
			},
			{
				skills: ["project-status", "project-recap"],
				body: "Where things stand and what's next; back after a pause.",
			},
		],
		guards:
			"Guard hooks keep the rules mechanical: no direct database writes, no secret values in the conversation, no force pushes, and a question before every merge, push or change to an issue.",
	},
	notFound: {
		title: "This page isn’t in the index.",
		body: "Search the skills, or start from the landing page.",
	},
	footer: {
		license: "GPL v3+",
		licenseTitle:
			"GNU General Public License v3.0 or later (GPL-3.0-or-later): read the license",
		version: "Versions",
		theme: "Theme",
		light: "Light theme",
		dark: "Dark theme",
	},
};

export const messages: Record<Locale, Messages> = {
	"en-US": enUS,
};

const STORAGE_KEY = "ai-gent-docs-locale";

function initialLocale(): Locale {
	const supported = LOCALES as readonly string[];
	if (typeof localStorage !== "undefined") {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored && supported.includes(stored)) return stored as Locale;
	}
	if (typeof navigator !== "undefined") {
		for (const tag of navigator.languages ?? [navigator.language]) {
			const match = LOCALES.find(
				(l) =>
					l.toLowerCase() === tag.toLowerCase() ||
					tag.toLowerCase().startsWith(`${l.slice(0, 2).toLowerCase()}-`),
			);
			if (match) return match;
		}
	}
	return LOCALES[0];
}

interface I18nValue {
	locale: Locale;
	t: Messages;
	setLocale: (locale: Locale) => void;
}

const I18nContext = createContext<I18nValue>({
	locale: "en-US",
	t: enUS,
	setLocale: () => {},
});

export function I18nProvider({ children }: { children: ReactNode }) {
	const [locale, setLocaleState] = useState<Locale>(initialLocale);

	useEffect(() => {
		document.documentElement.lang = locale;
		localStorage.setItem(STORAGE_KEY, locale);
	}, [locale]);

	const value: I18nValue = {
		locale,
		t: messages[locale],
		setLocale: setLocaleState,
	};
	return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
	return useContext(I18nContext);
}
