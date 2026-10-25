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
import { footerVersions, gitInfo, label } from "./version.mjs";

const siteDir = fileURLToPath(new URL("..", import.meta.url));
const repo = fileURLToPath(new URL("../../..", import.meta.url));

const REPO_URL = "https://github.com/lucasvmigotto/ai-gent";
const git = gitInfo(repo);
// Source links open the files this site was built from: the release tag,
// else the commit, else (no git) main.
const SOURCE_REF = git.exactTag || git.sha || "main";

// ai-gent's version is its release tag; the site's is docs/site/package.json,
// raised by scripts/release.py (release.json) when docs/site changes.
const versions = footerVersions({
	repoUrl: REPO_URL,
	name: "ai-gent",
	docsVersion: JSON.parse(readFileSync(join(siteDir, "package.json"), "utf8"))
		.version,
	docsPath: "docs/site",
	git,
}).map((v) => ({ ...v, label: label(v) }));

// Splits CHANGELOG.md's own "## Unreleased" / "## X.Y.Z — date" headings into
// one entry per release, newest first (the file's own order), each carrying
// its version parts so the site can fold the list by major, then minor.
const CHANGELOG_HEADING =
	/^## (?:Unreleased|(\d+)\.(\d+)\.(\d+))(?: — (\d{4}-\d{2}-\d{2}))?\s*$/;

function parseChangelog(text) {
	const entries = [];
	let current = null;
	for (const line of text.split("\n")) {
		const m = CHANGELOG_HEADING.exec(line);
		if (m) {
			if (current) entries.push(current);
			const [, major, minor, patch, date] = m;
			current =
				major === undefined
					? {
							version: "Unreleased",
							date: null,
							major: null,
							minor: null,
							patch: null,
							body: [],
						}
					: {
							version: `${major}.${minor}.${patch}`,
							date: date ?? null,
							major: Number(major),
							minor: Number(minor),
							patch: Number(patch),
							body: [],
						};
		} else if (current) {
			current.body.push(line);
		}
	}
	if (current) entries.push(current);
	return entries
		.map((e) => ({ ...e, body: e.body.join("\n").trim() }))
		.filter((e) => e.body.length > 0);
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
			sourceUrl: `${REPO_URL}/blob/${SOURCE_REF}/plugins/${plugin}/skills/${name}/SKILL.md`,
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
	version: versions[0].version,
	versions,
	sourceBase: `${REPO_URL}/blob/${SOURCE_REF}`,
	plugins,
	skills,
	counts: { plugins: plugins.length, skills: skills.length },
	readme: readFileSync(join(repo, "README.md"), "utf8"),
	changelog: readFileSync(join(repo, "CHANGELOG.md"), "utf8"),
	changelogVersions: parseChangelog(
		readFileSync(join(repo, "CHANGELOG.md"), "utf8"),
	),
};

const out = join(siteDir, "src/content/generated.json");
mkdirSync(join(siteDir, "src/content"), { recursive: true });
writeFileSync(out, `${JSON.stringify(content, null, 2)}\n`);
console.log(
	`generated ${out}: ${plugins.length} plugins, ${skills.length} skills, version ${content.version}`,
);
