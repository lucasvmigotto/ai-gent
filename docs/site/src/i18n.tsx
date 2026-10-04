import {
	createContext,
	type ReactNode,
	useContext,
	useEffect,
	useState,
} from "react";

export type Locale = "en-US" | "pt-BR";
export const LOCALES: Locale[] = ["en-US", "pt-BR"];

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
			"Install ai-gent, find the right skill among 36, and contribute — in English and Portuguese.",
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
			`${skills} skills · ${plugins} plugins · en-US + pt-BR`,
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
			"Two PreToolUse hooks enforce the git and db rules in Claude Code; an OpenCode V2 plugin enforces the same rules there.",
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

export const ptBR: Messages = {
	meta: {
		title: "ai-gent docs — skills e plugins para Claude Code e opencode",
		description:
			"Instale o ai-gent, encontre a skill certa entre 36 e contribua — em inglês e português.",
	},
	a11y: { skip: "Pular para o conteúdo", external: "abre em nova aba" },
	nav: {
		use: "Usar",
		skills: "Skills",
		contribute: "Contribuir",
		reference: "Referência",
		changelog: "Changelog",
		install: "Instalar",
		search: "Buscar skills",
		searchLabel: "Buscar skills",
		theme: "Alternar tema",
		language: "Idioma",
		menu: "Menu",
		close: "Fechar menu",
		home: "Início do ai-gent docs",
	},
	landing: {
		title: "Encontre a skill para a tarefa.",
		lede: "O ai-gent são 9 plugins e 36 skills para Claude Code e opencode. Digite o que você quer fazer.",
		placeholder: "O que você está tentando fazer?",
		counts: (plugins, skills) =>
			`${skills} skills · ${plugins} plugins · en-US + pt-BR`,
		installCta: "Instalar o ai-gent",
		changelogCta: "Ler o changelog",
		index: "Todas as skills, por plugin",
	},
	search: {
		emptyTitle: (query) => `Nenhuma skill corresponde a “${query}”.`,
		emptyBody: "Tente menos palavras ou navegue pelo índice completo.",
		results: (count) => `${count} skills correspondem`,
		clearing: "Busca limpa",
	},
	skills: {
		title: "Skills",
		lede: "Todas as skills, agrupadas por plugin.",
		filter: "Filtrar por plugin",
		all: "Todos os plugins",
	},
	skill: {
		when: "Quando invocar",
		copy: "Copiar",
		copied: "Copiado",
		generated: (plugin, version, site) =>
			`Gerado de ${plugin} v${version} · ai-gent ${site}`,
		source: "Ler o SKILL.md original",
		related: "Skills relacionadas",
		back: "Todas as skills",
		bodyNote:
			"As instruções abaixo são o texto original da skill, mantido em inglês.",
		invoke: "Invocar",
	},
	install: {
		title: "Instalar o ai-gent",
		lede: "Um comando por ferramenta. Rode de novo para atualizar.",
		choose: "Escolha sua ferramenta",
		claude: "Claude Code",
		opencode: "opencode",
		verify: "Verificar",
		firstSkill: "Invoque sua primeira skill",
		firstSkillBody:
			"Descreva o que você quer em palavras simples e a skill certa carrega, ou invoque uma diretamente.",
		troubleshooting: "Se não funcionar",
		next: "Próximo: ver as skills",
	},
	contribute: {
		title: "Contribuir",
		lede: "Altere uma skill ou plugin e entregue com tudo verde.",
		conventions: "Convenções",
		conventionsBody:
			"Mudanças seguem Conventional Commits; uma branch por contexto; descrições em uma linha, no máximo 400 caracteres, sem “: ” nem “ #”.",
		checks: "Rodar as verificações",
		checksBody:
			"scripts/check.sh valida metadados, referências, symlinks, scripts de shell e as suítes. A CI roda o mesmo script.",
		releases: "Releases",
		releasesBody:
			"Automático: pushes na main decidem o bump pelos commits, sobem a versão dos plugins alterados, promovem o changelog e criam a tag.",
		versions: "Versões",
		layout: "Estrutura",
		layoutBody:
			"Um plugin é plugins/<name>/ com um manifesto e skills/<skill>/SKILL.md. Código nativo do OpenCode fica em opencode/.",
	},
	reference: {
		title: "Referência",
		lede: "Os contratos e regras em que o toolkit roda.",
		pipeline: "Pipeline de produto",
		containers: "Contêineres",
		databases: "Bancos de dados",
		guards: "Guard hooks",
		readingCode: "Leitura de código",
		specKit: "Spec Kit",
		guardsBody:
			"Dois hooks PreToolUse aplicam as regras de git e db no Claude Code; um plugin do OpenCode V2 aplica as mesmas regras lá.",
	},
	changelog: { title: "Changelog", lede: "Releases e o que mudou." },
	notFound: {
		title: "Esta página não está no índice.",
		body: "Busque nas skills ou volte ao início.",
	},
	footer: {
		license: "GPL-3.0-or-later",
		source: "Código no GitHub",
		version: "versão do ai-gent",
	},
};

export const messages: Record<Locale, Messages> = {
	"en-US": enUS,
	"pt-BR": ptBR,
};

const STORAGE_KEY = "ai-gent-docs-locale";

function initialLocale(): Locale {
	if (typeof localStorage !== "undefined") {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored === "pt-BR" || stored === "en-US") return stored;
	}
	if (
		typeof navigator !== "undefined" &&
		navigator.language.toLowerCase().startsWith("pt")
	) {
		return "pt-BR";
	}
	return "en-US";
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
