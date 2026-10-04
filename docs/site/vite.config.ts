import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Served under a path prefix (https://docs.lucasvmigotto.me/ai-gent/); CI
// overrides both values from the repository name (see .env.example).
const basePath = process.env.VITE_BASE_PATH || "/ai-gent/";
const siteUrl =
	process.env.VITE_SITE_URL || "https://docs.lucasvmigotto.me/ai-gent";

function htmlEnv() {
	return {
		name: "ai-gent-html-env",
		transformIndexHtml(html: string) {
			return html
				.replaceAll("%SITE_URL%", siteUrl)
				.replaceAll("%VITE_BASE_PATH%", basePath);
		},
	};
}

export default defineConfig({
	base: basePath,
	plugins: [react(), tailwindcss(), htmlEnv()],
});
