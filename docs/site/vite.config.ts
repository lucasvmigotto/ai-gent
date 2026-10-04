import { execSync } from "node:child_process";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

function siteVersion(): string {
	const fromEnv = process.env.AI_GENT_VERSION?.trim();
	if (fromEnv) return fromEnv;
	try {
		return execSync("git describe --tags --abbrev=0", {
			encoding: "utf8",
			// docs/site/vite.config.ts -> repo root
			cwd: new URL("../..", import.meta.url),
		}).trim();
	} catch {
		return "dev";
	}
}

// Served under a path prefix (https://docs.lucasvmigotto.me/ai-gent/),
// so every asset resolves from /ai-gent/.
export default defineConfig({
	base: "/ai-gent/",
	plugins: [react(), tailwindcss()],
	define: {
		__AI_GENT_VERSION__: JSON.stringify(siteVersion()),
	},
});
