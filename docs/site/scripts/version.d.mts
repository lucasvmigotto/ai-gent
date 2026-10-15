// Types for version.mjs (plain JS so generate-content.mjs runs under node).
export interface GitInfo {
	exactTag: string;
	lastTag: string;
	sha: string;
}
export interface FooterVersion {
	name: string;
	version: string;
	href: string | null;
}
export function gitInfo(repo: string): GitInfo;
export function footerVersions(options: {
	repoUrl: string;
	name: string;
	docsVersion: string;
	docsPath: string;
	git: GitInfo;
}): FooterVersion[];
export function label(v: Pick<FooterVersion, "name" | "version">): string;
