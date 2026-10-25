import {
	Activity,
	BadgeCheck,
	BookOpen,
	Boxes,
	CircleCheck,
	Compass,
	Container,
	Database,
	Download,
	GitBranch,
	GitMerge,
	GitPullRequest,
	Hammer,
	Hand,
	KeyRound,
	Lightbulb,
	ListChecks,
	type LucideIcon,
	PenTool,
	RefreshCw,
	Rocket,
	ShieldCheck,
	Ticket,
	Workflow,
} from "lucide-react";
import { useState } from "react";
import { Link, useParams } from "react-router";
import {
	CodeBlock,
	Markdown,
	SearchBox,
	SectionBlocks,
	SkillIndex,
	SkillRow,
} from "./components";
import {
	changelogVersions,
	command,
	groupChangelog,
	pluginOf,
	plugins,
	skillById,
	skills,
	skillsOf,
	triggers,
	version,
} from "./content";
import {
	contributeSections,
	flowSections,
	installSections,
	referenceSections,
	sectionKey,
} from "./content/sections";
import { type FlowStep, type Messages, useI18n } from "./i18n";

export function LandingPage() {
	const { t } = useI18n();
	const [query, setQuery] = useState("");
	return (
		<>
			<section className="py-6">
				<h1 className="font-display text-4xl sm:text-5xl">{t.landing.title}</h1>
				<p className="mt-3 max-w-2xl text-muted">{t.landing.lede}</p>
				<p className="mt-2 max-w-2xl text-sm text-muted">{t.landing.why}</p>
				<p className="mt-2 text-sm text-muted">
					{t.landing.counts(plugins.length, skills.length)}
				</p>
				<div className="mt-6 max-w-2xl">
					<SearchBox query={query} onQuery={setQuery} />
				</div>
				<div className="mt-6 flex flex-wrap gap-3">
					<Link
						to="/use/install"
						className="inline-flex items-center gap-2 rounded-[0.375rem] bg-signal px-4 py-2 font-medium text-on-signal"
					>
						<Download size={18} aria-hidden="true" />
						{t.landing.installCta}
					</Link>
					<Link
						to="/use/flow"
						className="inline-flex items-center gap-2 rounded-[0.375rem] border border-muted/40 px-4 py-2"
					>
						<Workflow size={18} aria-hidden="true" />
						{t.landing.flowCta}
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
				<span>{command(skill)}</span>
			</nav>
			<h1 className="mt-2 font-display text-3xl">{skill.name}</h1>
			<p className="mt-2 max-w-3xl text-muted">{skill.description}</p>

			<h2 className="mt-6 font-display text-xl">{t.skill.invoke}</h2>
			<CodeBlock
				label="claude code · other agents"
				code={`${command(skill)}   # Claude Code\n/${skill.id}   # opencode, Cursor; $${skill.id} in Codex; by name elsewhere`}
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
			<SectionBlocks sections={installSections(t)} />
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
			<SectionBlocks sections={contributeSections(t, plugins)} />
		</>
	);
}

export function ReferencePage() {
	const { t } = useI18n();
	return (
		<>
			<h1 className="font-display text-3xl">{t.reference.title}</h1>
			<p className="mt-2 max-w-2xl text-muted">{t.reference.lede}</p>
			<SectionBlocks sections={referenceSections(t)} />
		</>
	);
}

function SkillChips({ ids }: { ids: string[] }) {
	return (
		<ul className="mt-2 flex flex-wrap gap-2">
			{ids.map((id) => {
				const s = skillById(id);
				return (
					<li key={id}>
						{s ? (
							<Link
								to={`/use/skills/${id}`}
								className="rounded-[0.375rem] border border-muted/30 px-2 py-0.5 font-code text-xs text-signal hover:bg-surface"
							>
								{command(s)}
							</Link>
						) : (
							<span className="font-code text-xs text-muted">{id}</span>
						)}
					</li>
				);
			})}
		</ul>
	);
}

// Icons are presentation, not copy: keyed by step, track position and the
// support item's first skill, so every locale shares them.
const STEP_ICONS: Record<string, LucideIcon> = {
	start: Lightbulb,
	design: PenTool,
	specify: ListChecks,
	track: Ticket,
	develop: GitBranch,
	land: GitMerge,
	watch: Activity,
	verify: BadgeCheck,
	close: CircleCheck,
	release: Rocket,
	document: BookOpen,
};
const TRACK_ICONS: LucideIcon[] = [Workflow, Hammer, Container];
const SUPPORT_ICONS: Record<string, LucideIcon> = {
	secrets: KeyRound,
	db: Database,
	devcontainer: Boxes,
	devsecops: ShieldCheck,
	repo: GitPullRequest,
	project: Compass,
};

function supportIcon(skills: string[]): LucideIcon {
	return SUPPORT_ICONS[skills[0]?.split("-")[0] ?? ""] ?? Compass;
}

function FlowStepItem({ step, n }: { step: FlowStep; n: number }) {
	const { t } = useI18n();
	return (
		<li
			id={`flow-${step.key}`}
			className="relative scroll-mt-20 pb-8 pl-12 last:pb-0"
		>
			<span
				aria-hidden="true"
				className="absolute top-0 left-0 flex h-8 w-8 items-center justify-center rounded-full border-2 border-signal bg-bg text-signal"
			>
				{(() => {
					const Icon = STEP_ICONS[step.key] ?? CircleCheck;
					return <Icon size={16} />;
				})()}
			</span>
			<h3 className="flex flex-wrap items-baseline gap-2 font-display text-xl">
				<span className="font-code text-sm text-muted">
					{String(n).padStart(2, "0")}
				</span>
				{step.title}
				{step.asks && (
					<span className="inline-flex items-center gap-1 self-center rounded-full border border-signal px-2 font-body text-xs text-signal">
						<Hand size={12} aria-hidden="true" />
						{t.flow.asks}
					</span>
				)}
			</h3>
			<SkillChips ids={step.skills} />
			<p className="mt-2 max-w-2xl text-sm text-muted">{step.body}</p>
			{step.track && (
				<div className="mt-4 rounded-[0.375rem] border border-muted/30 bg-surface p-4">
					<h4 className="text-sm font-medium">{t.flow.inEachTrack}</h4>
					<ol className="mt-2 space-y-3">
						{step.track.map((x, i) => {
							const Icon = TRACK_ICONS[i] ?? Workflow;
							return (
								<li key={x.title} className="border-l-2 border-muted/30 pl-3">
									<p className="flex items-center gap-2 text-sm">
										<Icon
											size={14}
											aria-hidden="true"
											className="text-signal"
										/>
										{x.title}
									</p>
									<SkillChips ids={x.skills} />
									<p className="mt-1 text-sm text-muted">{x.body}</p>
								</li>
							);
						})}
					</ol>
				</div>
			)}
			{step.loop && (
				<div className="mt-4 flex gap-3 rounded-[0.375rem] border border-danger/50 p-4">
					<RefreshCw
						size={18}
						aria-hidden="true"
						className="shrink-0 text-danger"
					/>
					<p className="text-sm">
						<span className="font-medium">{t.flow.whenRed}: </span>
						<span className="text-muted">{step.loop}</span>
					</p>
				</div>
			)}
		</li>
	);
}

// The steps at a glance: one line of icons joined by a stretching line,
// the name in a tooltip on hover or focus (and as the button's name).
function Glance({ steps }: { steps: FlowStep[] }) {
	return (
		<ol className="flex items-center">
			{steps.map((s, i) => {
				const Icon = STEP_ICONS[s.key] ?? CircleCheck;
				return (
					<li
						key={s.key}
						className={`flex items-center ${i > 0 ? "flex-1" : ""}`}
					>
						{i > 0 && (
							<span
								aria-hidden="true"
								className="h-px min-w-1 flex-1 bg-muted/40"
							/>
						)}
						<span className="group relative">
							<button
								type="button"
								aria-label={s.title}
								onClick={() =>
									document.getElementById(`flow-${s.key}`)?.scrollIntoView()
								}
								className={`flex h-8 w-8 items-center justify-center rounded-full border-2 bg-bg hover:bg-surface sm:h-10 sm:w-10 ${s.loop ? "border-danger" : "border-signal"}`}
							>
								<Icon size={16} aria-hidden="true" className="text-signal" />
							</button>
							{s.loop && (
								<RefreshCw
									size={12}
									aria-hidden="true"
									className="absolute -top-1 -right-1 rounded-full bg-bg text-danger"
								/>
							)}
							<span
								aria-hidden="true"
								className="pointer-events-none absolute top-full left-1/2 z-10 mt-2 -translate-x-1/2 whitespace-nowrap rounded-[0.375rem] border border-muted/30 bg-surface px-2 py-1 text-xs opacity-0 shadow transition-opacity group-focus-within:opacity-100 group-hover:opacity-100"
							>
								{s.title}
							</span>
						</span>
					</li>
				);
			})}
		</ol>
	);
}

// Each custom block keyed by flowSections' id, rendered with the icons and
// landmarks Markdown has no use for; the heading text and the sequence
// itself come from flowSections(t), the same list generate-llms.ts reads.
function FlowCustomBlock({ id, t }: { id: string; t: Messages }) {
	const f = t.flow;
	switch (id) {
		case "glance":
			return (
				<section aria-labelledby="glance" className="mt-6">
					<h2 id="glance" className="sr-only">
						{f.glance}
					</h2>
					<Glance steps={f.steps} />
				</section>
			);
		case "loop":
			return (
				<ol className="relative mt-6 before:absolute before:top-2 before:bottom-2 before:left-4 before:w-px before:bg-muted/30">
					{f.steps.map((s, i) => (
						<FlowStepItem key={s.key} step={s} n={i + 1} />
					))}
				</ol>
			);
		case "support":
			return (
				<>
					<ul className="mt-4 grid gap-4 sm:grid-cols-2">
						{f.support.map((x) => {
							const Icon = supportIcon(x.skills);
							return (
								<li
									key={x.skills.join()}
									className="flex gap-3 rounded-[0.375rem] border border-muted/30 p-4"
								>
									<Icon
										size={20}
										aria-hidden="true"
										className="mt-1 shrink-0 text-signal"
									/>
									<div>
										<SkillChips ids={x.skills} />
										<p className="mt-2 text-sm text-muted">{x.body}</p>
									</div>
								</li>
							);
						})}
					</ul>
					<p className="mt-4 max-w-2xl text-sm text-muted">{f.guards}</p>
				</>
			);
		default:
			return null;
	}
}

export function FlowPage() {
	const { t } = useI18n();
	const f = t.flow;
	return (
		<>
			<h1 className="font-display text-3xl">{f.title}</h1>
			<p className="mt-2 max-w-2xl text-muted">{f.lede}</p>

			{flowSections(t).map((s) => {
				const key = sectionKey(s);
				if (s.kind === "custom" && s.id === "glance")
					return <FlowCustomBlock key={key} id="glance" t={t} />;
				if (s.kind === "heading" && s.text === f.loop)
					return (
						<h2 key={key} className="mt-8 font-display text-2xl">
							{s.text}
						</h2>
					);
				if (s.kind === "custom" && s.id === "loop")
					return <FlowCustomBlock key={key} id="loop" t={t} />;
				if (s.kind === "heading" && s.text === f.autonomyTitle)
					return (
						<h2 key={key} className="mt-10 max-w-2xl font-display text-2xl">
							{s.text}
						</h2>
					);
				if (s.kind === "paragraph" && s.text === f.autonomy)
					return (
						<p key={key} className="mt-2 max-w-2xl text-sm text-muted">
							{s.text}
						</p>
					);
				if (s.kind === "heading" && s.text === f.supportTitle)
					return (
						<h2 key={key} className="mt-10 font-display text-2xl">
							{s.text}
						</h2>
					);
				if (s.kind === "custom" && s.id === "support")
					return <FlowCustomBlock key={key} id="support" t={t} />;
				return null;
			})}
		</>
	);
}

function ChangelogEntryBody({
	entry,
}: {
	entry: (typeof changelogVersions)[number];
}) {
	return (
		<article
			id={`v${entry.version.replace(/\./g, "-")}`}
			className="scroll-mt-24"
		>
			<h3 className="font-display text-base">
				{entry.version}
				{entry.date ? (
					<span className="ml-2 font-body text-sm text-muted">
						{entry.date}
					</span>
				) : null}
			</h3>
			<div className="prose prose-invert mt-1 max-w-3xl">
				<Markdown>{entry.body}</Markdown>
			</div>
		</article>
	);
}

export function ChangelogPage() {
	const { t } = useI18n();
	const unreleased = changelogVersions.find((e) => e.version === "Unreleased");
	const released = changelogVersions.filter((e) => e.version !== "Unreleased");
	const majors = groupChangelog(released);
	const latestMajor = majors[0]?.major;
	return (
		<>
			<h1 className="font-display text-3xl">{t.changelog.title}</h1>
			<p className="mt-2 text-muted">{t.changelog.lede}</p>
			<h2 id="changelog-releases" className="sr-only">
				{t.changelog.releases}
			</h2>
			{unreleased ? (
				<div className="mt-6 max-w-3xl">
					<ChangelogEntryBody entry={unreleased} />
				</div>
			) : null}
			<div className="mt-6 max-w-3xl space-y-3">
				{majors.map((g) => (
					<details
						key={g.major}
						open={g.major === latestMajor}
						className="rounded-[0.375rem] border border-muted/30"
					>
						<summary className="cursor-pointer px-4 py-3 font-display text-lg">
							{g.major}.x
						</summary>
						<div className="space-y-3 border-t border-muted/30 px-4 py-3">
							{g.minors.map((m) => (
								<details
									key={m.minor}
									open={g.major === latestMajor && m === g.minors[0]}
									className="rounded-[0.375rem] border border-muted/15"
								>
									<summary className="cursor-pointer px-3 py-2 font-medium">
										{g.major}.{m.minor}.x
									</summary>
									<div className="space-y-4 border-t border-muted/15 px-3 py-3">
										{m.entries.map((entry) => (
											<ChangelogEntryBody key={entry.version} entry={entry} />
										))}
									</div>
								</details>
							))}
						</div>
					</details>
				))}
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
