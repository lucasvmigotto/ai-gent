import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { Link, NavLink, useNavigate } from "react-router";
import remarkGfm from "remark-gfm";
import {
	counts,
	plugins,
	type SkillInfo,
	searchSkills,
	skills,
	version,
} from "./content";
import { LOCALES, type Locale, useI18n } from "./i18n";

// Rendered page bodies already own the h1, so Markdown headings are demoted
// one level to keep one h1 per page.
const DEMOTED_HEADINGS = {
	h1: "h2",
	h2: "h3",
	h3: "h4",
	h4: "h5",
	h5: "h6",
	h6: "h6",
} as const;

export function Markdown({ children }: { children: string }) {
	return (
		<ReactMarkdown remarkPlugins={[remarkGfm]} components={DEMOTED_HEADINGS}>
			{children}
		</ReactMarkdown>
	);
}

function ThemeToggle() {
	const { t } = useI18n();
	const [theme, setTheme] = useState(
		() => document.documentElement.dataset.theme ?? "dark",
	);
	useEffect(() => {
		document.documentElement.dataset.theme = theme;
		localStorage.setItem("ai-gent-docs-theme", theme);
	}, [theme]);
	return (
		<button
			type="button"
			className="rounded-[var(--radius-control,0.375rem)] px-2 py-1 text-sm hover:text-signal"
			onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
		>
			{t.nav.theme}
		</button>
	);
}

function LanguageSwitcher() {
	const { locale, setLocale, t } = useI18n();
	return (
		<label className="text-sm">
			<span className="sr-only">{t.nav.language}</span>
			<select
				className="rounded-[0.375rem] border border-muted/40 bg-surface px-2 py-1"
				value={locale}
				onChange={(e) => setLocale(e.target.value as Locale)}
			>
				{LOCALES.map((l) => (
					<option key={l} value={l}>
						{l}
					</option>
				))}
			</select>
		</label>
	);
}

function TopBar() {
	const { t } = useI18n();
	const [open, setOpen] = useState(false);
	const navClass = ({ isActive }: { isActive: boolean }) =>
		isActive ? "text-signal underline underline-offset-4" : "hover:text-signal";
	return (
		<header className="border-b border-muted/20">
			<div className="mx-auto flex max-w-6xl items-center gap-4 px-4 py-3">
				<Link to="/" className="font-display text-lg font-semibold">
					{t.nav.home}
				</Link>
				<VersionBadge />
				<nav aria-label="Primary" className="hidden gap-4 text-sm md:flex">
					<NavLink to="/use/install" className={navClass}>
						{t.nav.use}
					</NavLink>
					<NavLink to="/use/skills" className={navClass}>
						{t.nav.skills}
					</NavLink>
					<NavLink to="/contribute" className={navClass}>
						{t.nav.contribute}
					</NavLink>
					<NavLink to="/reference" className={navClass}>
						{t.nav.reference}
					</NavLink>
					<NavLink to="/changelog" className={navClass}>
						{t.nav.changelog}
					</NavLink>
				</nav>
				<div className="ml-auto flex items-center gap-3">
					<Link to="/use/skills" className="text-sm hover:text-signal">
						{t.nav.search}
					</Link>
					<LanguageSwitcher />
					<ThemeToggle />
					<button
						type="button"
						className="text-sm md:hidden"
						aria-expanded={open}
						onClick={() => setOpen(!open)}
					>
						{open ? t.nav.close : t.nav.menu}
					</button>
				</div>
			</div>
			{open && (
				<nav
					aria-label="Primary mobile"
					className="flex flex-col gap-2 px-4 pb-3 text-sm md:hidden"
				>
					<Link to="/use/install" onClick={() => setOpen(false)}>
						{t.nav.use}
					</Link>
					<Link to="/use/skills" onClick={() => setOpen(false)}>
						{t.nav.skills}
					</Link>
					<Link to="/contribute" onClick={() => setOpen(false)}>
						{t.nav.contribute}
					</Link>
					<Link to="/reference" onClick={() => setOpen(false)}>
						{t.nav.reference}
					</Link>
					<Link to="/changelog" onClick={() => setOpen(false)}>
						{t.nav.changelog}
					</Link>
				</nav>
			)}
		</header>
	);
}

export function VersionBadge() {
	return (
		<span className="font-code text-xs text-muted" title="ai-gent version">
			{version}
		</span>
	);
}

export function CodeBlock({ code, label }: { code: string; label?: string }) {
	const { t } = useI18n();
	const [copied, setCopied] = useState(false);
	const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const id = useId();

	async function copy() {
		try {
			await navigator.clipboard.writeText(code);
		} catch {
			/* clipboard unavailable; the text is still selectable */
		}
		setCopied(true);
		if (timer.current) clearTimeout(timer.current);
		timer.current = setTimeout(() => setCopied(false), 1500);
	}

	return (
		<div className="my-3 overflow-hidden rounded-[0.5rem] border border-muted/25 bg-surface">
			<div className="flex items-center justify-between border-b border-muted/20 px-3 py-1.5">
				<span className="font-code text-xs text-muted">{label ?? "shell"}</span>
				<button
					type="button"
					className="text-xs hover:text-signal"
					onClick={copy}
				>
					{t.skill.copy}
				</button>
			</div>
			<pre className="overflow-x-auto px-3 py-2">
				<code className="font-code text-sm">{code}</code>
			</pre>
			<p id={id} aria-live="polite" className="sr-only">
				{copied ? t.skill.copied : ""}
			</p>
		</div>
	);
}

export function SkillRow({ skill }: { skill: SkillInfo }) {
	const { t } = useI18n();
	return (
		<li className="border-t border-muted/15">
			<Link
				to={`/use/skills/${skill.id}`}
				className="flex flex-col gap-1 px-1 py-3 hover:bg-surface"
			>
				<span className="font-code text-sm text-signal">{`/${skill.id}`}</span>
				<span className="text-sm text-muted">
					{skill.description.split(". ")[0]}
				</span>
			</Link>
			<span className="sr-only">{t.skills.title}</span>
		</li>
	);
}

export function SearchBox({
	query,
	onQuery,
}: {
	query: string;
	onQuery: (q: string) => void;
}) {
	const { t } = useI18n();
	const inputRef = useRef<HTMLInputElement>(null);
	const results = searchSkills(query);

	// "/" focuses search, the vision's keyboard trigger (ignored while typing).
	useEffect(() => {
		function onKey(e: KeyboardEvent) {
			if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
			const el = document.activeElement;
			const typing =
				el instanceof HTMLInputElement ||
				el instanceof HTMLTextAreaElement ||
				el instanceof HTMLSelectElement ||
				(el instanceof HTMLElement && el.isContentEditable);
			if (typing) return;
			e.preventDefault();
			inputRef.current?.focus();
		}
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	}, []);

	return (
		<div>
			<label className="block">
				<span className="sr-only">{t.nav.searchLabel}</span>
				<input
					ref={inputRef}
					type="search"
					value={query}
					onChange={(e) => onQuery(e.target.value)}
					placeholder={t.landing.placeholder}
					className="w-full rounded-[0.375rem] border border-muted/40 bg-surface px-3 py-2 text-base"
				/>
			</label>
			<p aria-live="polite" className="mt-2 text-sm text-muted">
				{query ? t.search.results(results.length) : ""}
			</p>
			{query && results.length === 0 && (
				<div className="mt-2 text-sm">
					<p className="font-medium">{t.search.emptyTitle(query)}</p>
					<p className="text-muted">{t.search.emptyBody}</p>
				</div>
			)}
			{query && results.length > 0 && (
				<ul className="mt-2">
					{results.map((s) => (
						<SkillRow key={s.id} skill={s} />
					))}
				</ul>
			)}
		</div>
	);
}

export function SkillIndex({ plugin }: { plugin?: string }) {
	const grouped = plugins
		.filter((p) => !plugin || p.name === plugin)
		.map((p) => ({
			plugin: p,
			list: skills.filter((s) => s.plugin === p.name),
		}));
	return (
		<div className="space-y-6">
			{grouped.map(({ plugin: p, list }) => (
				<section key={p.name} aria-labelledby={`plugin-${p.name}`}>
					<h3 id={`plugin-${p.name}`} className="font-display text-lg">
						{p.name}{" "}
						<span className="font-code text-xs text-muted">v{p.version}</span>
					</h3>
					<ul>
						{list.map((s) => (
							<SkillRow key={s.id} skill={s} />
						))}
					</ul>
				</section>
			))}
		</div>
	);
}

export function Layout({ children }: { children: ReactNode }) {
	const { t } = useI18n();
	return (
		<>
			<a
				href="#main"
				className="sr-only focus:not-sr-only focus:absolute focus:left-2 focus:top-2 focus:bg-surface focus:p-2"
			>
				{t.a11y.skip}
			</a>
			<TopBar />
			<main id="main" className="mx-auto max-w-6xl px-4 py-8">
				{children}
			</main>
			<footer className="border-t border-muted/20 px-4 py-6 text-sm text-muted">
				<div className="mx-auto flex max-w-6xl flex-wrap gap-4">
					<span>
						{t.footer.version} {version}
					</span>
					<span>{t.footer.license}</span>
					<a
						href="https://github.com/lucasvmigotto/ai-gent"
						rel="noreferrer"
						className="hover:text-signal"
					>
						{t.footer.source}
					</a>
					<span>
						{counts.plugins} plugins · {counts.skills} skills
					</span>
				</div>
			</footer>
		</>
	);
}

export { useNavigate };
