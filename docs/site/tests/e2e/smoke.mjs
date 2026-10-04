// End-to-end smoke for the built site, driven by a containerized browser.
//
// The browser comes from the selenium/standalone-* image (see the CI job and
// docs/product/ux-vision.md); nothing installs a driver on the host. Run:
//   bun run build
//   bunx vite preview --host 127.0.0.1 --port 4173 --strictPort &
//   SITE_URL=http://127.0.0.1:4173/ai-gent/ \
//     SELENIUM_URL=http://127.0.0.1:4444 bun run e2e
// The browser must share the host network (--network host): a container
// cannot reach the host's 127.0.0.1 otherwise.
import { Builder, By, until } from "selenium-webdriver";

const base = process.env.SITE_URL || "http://127.0.0.1:4173/ai-gent/";
const server = process.env.SELENIUM_URL || "http://localhost:4444";

function assert(ok, message) {
	if (!ok) throw new Error(message);
}

async function main() {
	const driver = await new Builder()
		.usingServer(server)
		.forBrowser("chrome")
		.build();
	try {
		// Landing renders at the router root (guards the prefix-basename bug).
		await driver.get(base);
		await driver.wait(until.elementLocated(By.css("h1")), 10_000);
		const landing = await driver.findElement(By.css("h1")).getText();
		assert(/find the skill/i.test(landing), `landing h1 was: ${landing}`);

		// A skill page renders its invoke commands.
		await driver.get(`${base}#/use/skills/qa-load`);
		await driver.wait(until.elementLocated(By.css("h1")), 10_000);
		const heading = await driver.findElement(By.css("h1")).getText();
		assert(/load/i.test(heading), `skill h1 was: ${heading}`);
		const body = await driver.findElement(By.css("body")).getText();
		assert(
			body.includes("/qa:load"),
			"skill page is missing the Claude Code invoke command",
		);
		assert(
			body.includes("/qa-load"),
			"skill page is missing the opencode invoke command",
		);

		// An unknown route renders the not-found state, not a blank page.
		await driver.get(`${base}#/does-not-exist`);
		await driver.wait(until.elementLocated(By.css("h1")), 10_000);
		const notFound = await driver.findElement(By.css("h1")).getText();
		assert(/in the index/i.test(notFound), `404 h1 was: ${notFound}`);

		console.log("e2e smoke ok: landing, skill page and not-found");
	} finally {
		await driver.quit();
	}
}

main().catch((error) => {
	console.error(error);
	process.exit(1);
});
