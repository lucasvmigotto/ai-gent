import { useState } from "react";
import { Link, useParams } from "react-router";
import {
	CodeBlock,
	Markdown,
	SearchBox,
	SkillIndex,
	SkillRow,
} from "./components";
import {
	changelog as changelogText,
	pluginOf,
	plugins,
	skillById,
	skills,
	skillsOf,
	triggers,
	version,
} from "./content";
import { useI18n } from "./i18n";

export function LandingPage() {
	const { t } = useI18n();
	const [query, setQuery] = useState("");
	return (
		<>
			<section className="py-6">
				<h1 className="font-display text-4xl sm:text-5xl">{t.landing.title}</h1>
				<p className="mt-3 max-w-2xl text-muted">{t.landing.lede}</p>
				<p className="mt-2 text-sm text-muted">
					{t.landing.counts(plugins.length, skills.length)}
				</p>
				<div className="mt-6 max-w-2xl">
					<SearchBox query={query} onQuery={setQuery} />
				</div>
				<div className="mt-6 flex flex-wrap gap-3">
					<Link
						to="/use/install"
						className="rounded-[0.375rem] bg-signal px-4 py-2 font-medium text-on-signal"
					>
						{t.landing.installCta}
					</Link>
					<Link
						to="/changelog"
						className="rounded-[0.375rem] border border-muted/40 px-4 py-2"
					>
						{t.landing.changelogCta}
					</Link>
				</div>
			</section>
			<section aria-labelledby="index" className="py-4">
				<h2 id="index" className="font-display text-2xl">
					{t.landing.index}
				</h2>
				<div className="mt-4">
					<SkillIndex />
				</div>
			</section>
		</>
	);
}

export function SkillsPage() {
	const { t } = useI18n();
	const [query, setQuery] = useState("");
	const [plugin, setPlugin] = useState("");
	return (
		<>
			<h1 className="font-display text-3xl">{t.skills.title}</h1>
			<p className="mt-2 text-muted">{t.skills.lede}</p>
			<div className="mt-4 max-w-xl">
				<SearchBox query={query} onQuery={setQuery} />
			</div>
			<div className="mt-4">
				<label className="text-sm">
					<span className="mr-2 text-muted">{t.skills.filter}</span>
					<select
						className="rounded-[0.375rem] border border-muted/40 bg-surface px-2 py-1"
						value={plugin}
						onChange={(e) => setPlugin(e.target.value)}
					>
						<option value="">{t.skills.all}</option>
						{plugins.map((p) => (
							<option key={p.name} value={p.name}>
								{p.name}
							</option>
						))}
					</select>
				</label>
			</div>
			<div className="mt-6">
				<SkillIndex plugin={plugin || undefined} />
			</div>
		</>
	);
}

export function SkillPage() {
	const { t } = useI18n();
	const { id } = useParams();
	const skill = id ? skillById(id) : undefined;
	if (!skill) {
		return (
			<>
				<h1 className="font-display text-3xl">{t.notFound.title}</h1>
				<p className="mt-2 text-muted">{t.notFound.body}</p>
				<Link
					to="/use/skills"
					className="mt-4 inline-block text-signal underline"
				>
					{t.skill.back}
				</Link>
			</>
		);
	}
	const plugin = pluginOf(skill);
	const related = skillsOf(skill.plugin).filter((s) => s.id !== skill.id);
	const when = triggers(skill.description);
	return (
		<article>
			<nav aria-label="Breadcrumb" className="text-sm text-muted">
				<Link to="/use/skills">{t.skills.title}</Link> /{" "}
				<span>/{skill.id}</span>
			</nav>
			<h1 className="mt-2 font-display text-3xl">{skill.name}</h1>
			<p className="mt-2 max-w-3xl text-muted">{skill.description}</p>

			<h2 className="mt-6 font-display text-xl">{t.skill.invoke}</h2>
			<CodeBlock
				label="claude code · opencode"
				code={`/${skill.plugin}:${skill.skill}   # Claude Code\n/${skill.id}   # opencode`}
			/>

			{when.length > 0 && (
				<>
					<h2 className="mt-6 font-display text-xl">{t.skill.when}</h2>
					<ul className="mt-2 list-inside list-disc text-sm text-muted">
						{when.map((w) => (
							<li key={w}>{w}</li>
						))}
					</ul>
				</>
			)}

			<p className="mt-4 font-code text-xs text-muted">
				{plugin && t.skill.generated(plugin.name, plugin.version, version)}
			</p>
			<a
				href={skill.sourceUrl}
				rel="noreferrer"
				className="text-sm text-signal underline"
			>
				{t.skill.source}
			</a>

			<h2 className="mt-8 font-display text-xl">{t.skill.bodyNote}</h2>
			<div className="prose prose-invert mt-2 max-w-3xl">
				<Markdown>{skill.body}</Markdown>
			</div>

			{related.length > 0 && (
				<>
					<h2 className="mt-8 font-display text-xl">{t.skill.related}</h2>
					<ul>
						{related.map((s) => (
							<SkillRow key={s.id} skill={s} />
						))}
					</ul>
				</>
			)}
		</article>
	);
}

export function InstallPage() {
	const { t } = useI18n();
	return (
		<>
			<h1 className="font-display text-3xl">{t.install.title}</h1>
			<p className="mt-2 max-w-2xl text-muted">{t.install.lede}</p>
			<h2 className="mt-6 font-display text-xl">{t.install.choose}</h2>
			<h3 className="mt-4 font-medium">{t.install.claude}</h3>
			<CodeBlock
				label="shell"
				code={`curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh -s -- --target claude`}
			/>
			<h3 className="mt-4 font-medium">{t.install.opencode}</h3>
			<CodeBlock
				label="shell"
				code={`curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh -s -- --target opencode`}
			/>
			<h2 className="mt-6 font-display text-xl">{t.install.verify}</h2>
			<CodeBlock
				label="shell"
				code={`claude plugin details project@skills-dir\nopencode plugin list`}
			/>
			<h2 className="mt-6 font-display text-xl">{t.install.firstSkill}</h2>
			<p className="mt-2 max-w-2xl text-muted">{t.install.firstSkillBody}</p>
			<p className="mt-4">
				<Link to="/use/skills" className="text-signal underline">
					{t.install.next}
				</Link>
			</p>
		</>
	);
}

export function ContributePage() {
	const { t } = useI18n();
	return (
		<>
			<h1 className="font-display text-3xl">{t.contribute.title}</h1>
			<p className="mt-2 max-w-2xl text-muted">{t.contribute.lede}</p>
			<section className="mt-6">
				<h2 className="font-display text-xl">{t.contribute.conventions}</h2>
				<p className="mt-1 max-w-2xl text-sm text-muted">
					{t.contribute.conventionsBody}
				</p>
			</section>
			<section className="mt-6">
				<h2 className="font-display text-xl">{t.contribute.checks}</h2>
				<p className="mt-1 max-w-2xl text-sm text-muted">
					{t.contribute.checksBody}
				</p>
				<CodeBlock label="shell" code="./scripts/check.sh" />
			</section>
			<section className="mt-6">
				<h2 className="font-display text-xl">{t.contribute.layout}</h2>
				<p className="mt-1 max-w-2xl text-sm text-muted">
					{t.contribute.layoutBody}
				</p>
			</section>
			<section className="mt-6">
				<h2 className="font-display text-xl">{t.contribute.releases}</h2>
				<p className="mt-1 max-w-2xl text-sm text-muted">
					{t.contribute.releasesBody}
				</p>
			</section>
			<section className="mt-6">
				<h2 className="font-display text-xl">{t.contribute.versions}</h2>
				<table className="mt-2 w-full max-w-xl text-left text-sm">
					<thead>
						<tr className="border-b border-muted/30">
							<th scope="col" className="py-1">
								plugin
							</th>
							<th scope="col" className="py-1">
								version
							</th>
						</tr>
					</thead>
					<tbody>
						{plugins.map((p) => (
							<tr key={p.name} className="border-b border-muted/15">
								<td className="py-1 font-code">{p.name}</td>
								<td className="py-1">{p.version}</td>
							</tr>
						))}
					</tbody>
				</table>
			</section>
		</>
	);
}

export function ReferencePage() {
	const { t } = useI18n();
	const entries = [
		{
			key: "pipeline",
			title: t.reference.pipeline,
			path: "shared/pipeline.md",
		},
		{
			key: "containers",
			title: t.reference.containers,
			path: "shared/containers.md",
		},
		{ key: "guards", title: t.reference.guards, body: t.reference.guardsBody },
		{
			key: "databases",
			title: t.reference.databases,
			path: "plugins/db/references/safety.md",
		},
		{
			key: "reading-code",
			title: t.reference.readingCode,
			path: "shared/reading-code.md",
		},
		{ key: "spec-kit", title: t.reference.specKit, path: "shared/spec-kit.md" },
	];
	return (
		<>
			<h1 className="font-display text-3xl">{t.reference.title}</h1>
			<p className="mt-2 max-w-2xl text-muted">{t.reference.lede}</p>
			<dl className="mt-6 space-y-4">
				{entries.map((e) => (
					<div key={e.key}>
						<dt className="font-display text-xl">{e.title}</dt>
						<dd className="mt-1 text-sm text-muted">
							{e.body ?? ""}
							{e.path && (
								<a
									className="ml-1 text-signal underline"
									href={`https://github.com/lucasvmigotto/ai-gent/blob/main/${e.path}`}
									rel="noreferrer"
								>
									{e.path}
								</a>
							)}
						</dd>
					</div>
				))}
			</dl>
		</>
	);
}

export function ChangelogPage() {
	const { t } = useI18n();
	return (
		<>
			<h1 className="font-display text-3xl">{t.changelog.title}</h1>
			<p className="mt-2 text-muted">{t.changelog.lede}</p>
			<div className="prose prose-invert mt-4 max-w-3xl">
				<Markdown>{changelogText}</Markdown>
			</div>
		</>
	);
}

export function NotFoundPage() {
	const { t } = useI18n();
	return (
		<>
			<h1 className="font-display text-3xl">{t.notFound.title}</h1>
			<p className="mt-2 text-muted">{t.notFound.body}</p>
			<Link to="/" className="mt-4 inline-block text-signal underline">
				{`/${""}`}
				{t.nav.home}
			</Link>
		</>
	);
}
