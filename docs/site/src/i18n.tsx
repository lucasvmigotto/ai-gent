import {
	createContext,
	type ReactNode,
	useContext,
	useEffect,
	useState,
} from "react";

export const LOCALES = ["en-US"] as const;
export type Locale = (typeof LOCALES)[number];

export interface Messages {
	meta: { title: string; description: string };
	a11y: { skip: string; external: string };
	nav: {
		use: string;
		skills: string;
		contribute: string;
		reference: string;
		changelog: string;
		install: string;
		search: string;
		searchLabel: string;
		theme: string;
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
		installCta: string;
		changelogCta: string;
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
		opencode: string;
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
		guardsBody: string;
	};
	changelog: { title: string; lede: string };
	notFound: { title: string; body: string };
	footer: { license: string; source: string; version: string };
}

export const enUS: Messages = {
	meta: {
		title: "ai-gent docs — skills and plugins for Claude Code and opencode",
		description:
			"Install ai-gent, find the right skill among 36, and contribute.",
	},
	a11y: { skip: "Skip to content", external: "opens in a new tab" },
	nav: {
		use: "Use",
		skills: "Skills",
		contribute: "Contribute",
		reference: "Reference",
		changelog: "Changelog",
		install: "Install",
		search: "Search skills",
		searchLabel: "Search skills",
		theme: "Toggle theme",
		language: "Language",
		menu: "Menu",
		close: "Close menu",
		home: "ai-gent docs home",
	},
	landing: {
		title: "Find the skill for the task.",
		lede: "ai-gent is 9 plugins and 36 skills for Claude Code and opencode. Type what you are trying to do.",
		placeholder: "What are you trying to do?",
		counts: (plugins, skills) =>
			`${skills} skills · ${plugins} plugins · ${LOCALES.join(" + ")}`,
		installCta: "Install ai-gent",
		changelogCta: "Read the changelog",
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
		lede: "One command per tool. Re-run it to update.",
		choose: "Choose your tool",
		claude: "Claude Code",
		opencode: "opencode",
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
		guardsBody:
			"Two PreToolUse hooks enforce the git and db rules in Claude Code; an OpenCode V2 plugin enforces the same rules there. The git plugin also runs a SessionStart check that prints one line when a newer ai-gent release is known.",
	},
	changelog: { title: "Changelog", lede: "Releases and what changed." },
	notFound: {
		title: "This page isn’t in the index.",
		body: "Search the skills, or start from the landing page.",
	},
	footer: {
		license: "GPL-3.0-or-later",
		source: "Source on GitHub",
		version: "ai-gent version",
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
