// The install commands the Install page shows, each with the repository file
// and text that backs it (FR-001, FR-003). tests/install-drift.test.ts reads
// every source and fails when a command or its backing text is gone, so the
// page cannot describe a flag the installer no longer has. Command text is
// never translated; the prose around it lives in ../i18n.tsx.

const BASE =
	"https://github.com/lucasvmigotto/ai-gent/releases/latest/download";

export interface InstallFact {
	id: string;
	/** The exact command (one or more lines) as shown. */
	command: string;
	/** Repository-relative file that documents or implements it. */
	source: string;
	/** Every string must occur in `source`. */
	needles: string[];
}

export const REPO_SLUG = "lucasvmigotto/ai-gent";

export const installFacts = {
	install: {
		id: "install",
		command: `curl -fsSL ${BASE}/install.sh | sh`,
		source: "README.md",
		needles: [`curl -fsSL ${BASE}/install.sh | sh`],
	},
	targets: {
		id: "targets",
		command: `curl -fsSL ${BASE}/install.sh | sh -s -- --target claude,opencode,codex`,
		source: "README.md",
		needles: ["--target claude,opencode,codex"],
	},
	inspect: {
		id: "inspect",
		command: `curl -fsSLo install.sh ${BASE}/install.sh\nless install.sh && sh install.sh`,
		source: "README.md",
		needles: ["less install.sh && sh install.sh"],
	},
	pin: {
		id: "pin",
		command: `curl -fsSL ${BASE}/install.sh | VERSION=2.26.0 sh`,
		source: "install.sh",
		needles: ["VERSION=2.26.0 sh install.sh", "pinned"],
	},
	unpin: {
		id: "unpin",
		command: `curl -fsSL ${BASE}/install.sh | VERSION=latest sh`,
		source: "install.sh",
		needles: ["VERSION", "latest (default)"],
	},
	list: {
		id: "list",
		command: "sh ~/.local/share/ai-gent/current/install.sh --list",
		source: "README.md",
		needles: ["install.sh --list"],
	},
	rollback: {
		id: "rollback",
		command: "sh ~/.local/share/ai-gent/current/install.sh --rollback",
		source: "README.md",
		needles: ["install.sh --rollback"],
	},
	prune: {
		id: "prune",
		command: "sh ~/.local/share/ai-gent/current/install.sh --prune",
		source: "README.md",
		needles: ["install.sh --prune"],
	},
	verifyAttestation: {
		id: "verifyAttestation",
		command:
			"sh ~/.local/share/ai-gent/current/install.sh --verify-attestation",
		source: "README.md",
		needles: ["install.sh --verify-attestation"],
	},
	verifyManual: {
		id: "verifyManual",
		command: `gh attestation verify <file> -R ${REPO_SLUG}`,
		source: "install.sh",
		needles: ["gh attestation verify", `SLUG="${REPO_SLUG}"`, '-R "$SLUG"'],
	},
	doctor: {
		id: "doctor",
		command: "ai-gent doctor",
		source: "rust/crates/ai-gent/src/cmd/doctor.rs",
		needles: ["pub struct DoctorArgs"],
	},
	updateCheck: {
		id: "updateCheck",
		command: "ai-gent update --check",
		source: "rust/crates/ai-gent/src/cmd/update.rs",
		needles: ["pub check: bool", "never installs"],
	},
	update: {
		id: "update",
		command: "ai-gent update",
		source: "rust/crates/ai-gent/src/cmd/update.rs",
		needles: ["pub struct UpdateArgs"],
	},
	updateRollback: {
		id: "updateRollback",
		command: "ai-gent update --rollback",
		source: "rust/crates/ai-gent/src/cmd/update.rs",
		needles: ["pub rollback: bool"],
	},
	migrate: {
		id: "migrate",
		command: `mv ~/.local/share/ai-gent ~/.local/share/ai-gent.clone\ncurl -fsSL ${BASE}/install.sh | sh -s -- --migrate`,
		source: "README.md",
		needles: [
			"mv ~/.local/share/ai-gent ~/.local/share/ai-gent.clone",
			"sh -s -- --migrate",
		],
	},
	verifyClaude: {
		id: "verifyClaude",
		command: "claude plugin details project@skills-dir\nopencode plugin list",
		source: "README.md",
		needles: ["claude plugin details", "opencode plugin list"],
	},
} satisfies Record<string, InstallFact>;
