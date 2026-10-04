// Generates the site's content from the repository it documents.
// Truth-first: every skill page comes from the real SKILL.md, so docs
// cannot drift from code. Run by `bun run gen` (before dev, typecheck,
// test and build).
import {
	mkdirSync,
	readdirSync,
	readFileSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const siteDir = fileURLToPath(new URL("..", import.meta.url));
const repo = fileURLToPath(new URL("../../..", import.meta.url));

function version() {
	// CI sets AI_GENT_VERSION from the release tag; local builds show "dev".
	return process.env.AI_GENT_VERSION?.trim() || "dev";
}

function frontmatter(text) {
	const m = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/.exec(text);
	if (!m) return { meta: {}, body: text };
	const meta = {};
	for (const line of m[1].split("\n")) {
		const kv = /^([a-z_]+):\s?(.*)$/.exec(line);
		if (kv) meta[kv[1]] = kv[2];
	}
	return { meta, body: m[2].trim() };
}

const plugins = [];
const skills = [];
const pluginDirs = readdirSync(join(repo, "plugins"), { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name)
	.sort();

for (const plugin of pluginDirs) {
	const manifestPath = join(
		repo,
		"plugins",
		plugin,
		".claude-plugin/plugin.json",
	);
	if (!statSync(manifestPath, { throwIfNoEntry: false })) continue;
	const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
	const skillsDir = join(repo, "plugins", plugin, "skills");
	const names = readdirSync(skillsDir, { withFileTypes: true })
		.filter((d) => d.isDirectory())
		.map((d) => d.name)
		.sort();

	const pluginSkills = [];
	for (const name of names) {
		const file = join(skillsDir, name, "SKILL.md");
		if (!statSync(file, { throwIfNoEntry: false })) continue;
		const { meta, body } = frontmatter(readFileSync(file, "utf8"));
		const id = `${plugin}-${name}`;
		pluginSkills.push(id);
		skills.push({
			id,
			plugin,
			skill: name,
			name: meta.name ?? name,
			description: meta.description ?? "",
			body,
			sourcePath: `plugins/${plugin}/skills/${name}/SKILL.md`,
			sourceUrl: `https://github.com/lucasvmigotto/ai-gent/blob/main/plugins/${plugin}/skills/${name}/SKILL.md`,
		});
	}
	plugins.push({
		name: plugin,
		dir: `plugins/${plugin}`,
		version: manifest.version,
		description: manifest.description,
		skills: pluginSkills,
	});
}

const content = {
	version: version(),
	plugins,
	skills,
	counts: { plugins: plugins.length, skills: skills.length },
	readme: readFileSync(join(repo, "README.md"), "utf8"),
	changelog: readFileSync(join(repo, "CHANGELOG.md"), "utf8"),
};

const out = join(siteDir, "src/content/generated.json");
mkdirSync(join(siteDir, "src/content"), { recursive: true });
writeFileSync(out, `${JSON.stringify(content, null, 2)}\n`);
console.log(
	`generated ${out}: ${plugins.length} plugins, ${skills.length} skills, version ${content.version}`,
);
