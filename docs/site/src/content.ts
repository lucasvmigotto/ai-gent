import generated from "./content/generated.json";

export interface PluginInfo {
	name: string;
	dir: string;
	version: string;
	description: string;
	skills: string[];
}

export interface SkillInfo {
	id: string;
	plugin: string;
	skill: string;
	name: string;
	description: string;
	body: string;
	sourcePath: string;
	sourceUrl: string;
}

// One per versioned part, then the docs site: `ai-gent@v1.12.0 · docs@v0.1.0`,
// each linked to the tree it was built from (href is null without git).
export interface VersionInfo {
	name: string;
	version: string;
	label: string;
	href: string | null;
}

export const version: string = generated.version;
export const versions: VersionInfo[] = generated.versions as VersionInfo[];

// A repository file at the ref this site was built from (tag, else commit).
export function sourceUrl(path: string): string {
	return `${generated.sourceBase}/${path}`;
}
export const plugins: PluginInfo[] = generated.plugins as PluginInfo[];
export const skills: SkillInfo[] = generated.skills as SkillInfo[];
export const counts = generated.counts;
export const changelog: string = generated.changelog;

// How a skill is shown: Claude Code's `/plugin:skill`, the default agent.
// The `plugin-skill` id stays the URL slug and the other agents' name.
export function command(skill: Pick<SkillInfo, "plugin" | "skill">): string {
	return `/${skill.plugin}:${skill.skill}`;
}

const byId = new Map(skills.map((s) => [s.id, s]));
const pluginByName = new Map(plugins.map((p) => [p.name, p]));

export function skillById(id: string): SkillInfo | undefined {
	return byId.get(id);
}

export function pluginOf(skill: SkillInfo): PluginInfo | undefined {
	return pluginByName.get(skill.plugin);
}

export function skillsOf(plugin: string): SkillInfo[] {
	return skills.filter((s) => s.plugin === plugin);
}

/** Search titles, descriptions, ids and plugin names, locale-agnostic over
 *  the English source (skill descriptions are the toolkit's canonical text). */
export function searchSkills(query: string): SkillInfo[] {
	const q = query.trim().toLowerCase();
	if (!q) return [];
	const terms = q.split(/\s+/);
	return skills
		.map((s) => {
			const haystack =
				`${command(s)} ${s.id} ${s.name} ${s.plugin} ${s.description}`.toLowerCase();
			const score = terms.reduce(
				(acc, term) => (haystack.includes(term) ? acc + 1 : acc),
				0,
			);
			return { s, score };
		})
		.filter((r) => r.score === terms.length && r.score > 0)
		.sort((a, b) => b.score - a.score || a.s.id.localeCompare(b.s.id))
		.map((r) => r.s);
}

/** Extracts the "Use for ..." trigger phrases from a skill description. */
export function triggers(description: string): string[] {
	const matches = description.match(/"(?:[^"\\]|\\.)*"/g);
	if (!matches) return [];
	return matches.map((m) => m.slice(1, -1)).slice(0, 6);
}
