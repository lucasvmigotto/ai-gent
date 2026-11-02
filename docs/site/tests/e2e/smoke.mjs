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
import { Builder, By, Key, until } from "selenium-webdriver";

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

		// The autonomous flow page renders its steps.
		await driver.get(`${base}#/use/flow`);
		await driver.wait(until.elementLocated(By.css("h1")), 10_000);
		const flow = await driver.findElement(By.css("h1")).getText();
		assert(/autonomous flow/i.test(flow), `flow h1 was: ${flow}`);

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

		// The Install page: heading, the install command and a copy button.
		await driver.get(`${base}#/use/install`);
		await driver.wait(until.elementLocated(By.css("h1")), 10_000);
		const install = await driver.findElement(By.css("h1")).getText();
		assert(/install ai-gent/i.test(install), `install h1 was: ${install}`);
		const installBody = await driver.findElement(By.css("body")).getText();
		assert(
			installBody.includes("releases/latest/download/install.sh | sh"),
			"install page is missing the install command",
		);
		const copy = await driver.findElements(By.css("main button"));
		assert(copy.length > 0, "install page has no copy button");

		// The skip link works on a deep link: focus moves to #main, same route.
		await driver.findElement(By.css("body")).sendKeys(Key.TAB);
		const skip = await driver.switchTo().activeElement();
		assert(
			/skip to content/i.test(await skip.getText()),
			"the skip link is not the first Tab stop",
		);
		await skip.sendKeys(Key.ENTER);
		const focused = await driver.executeScript(
			"return document.activeElement && document.activeElement.id",
		);
		assert(focused === "main", `skip link focused: ${focused}`);
		const still = await driver.findElement(By.css("h1")).getText();
		assert(/install ai-gent/i.test(still), `skip link left the page: ${still}`);

		// The compatibility matrix is a table with column and row headers.
		await driver.get(`${base}#/use/compatibility`);
		await driver.wait(until.elementLocated(By.css("table")), 10_000);
		const cols = await driver.findElements(By.css("table th[scope=col]"));
		const rows = await driver.findElements(By.css("table th[scope=row]"));
		assert(cols.length >= 4, `matrix has ${cols.length} column headers`);
		assert(rows.length >= 8, `matrix has ${rows.length} row headers`);

		// Keyboard search on the landing page: type, arrow down, Enter.
		await driver.get(base);
		const box = await driver.wait(
			until.elementLocated(By.css("input[role=combobox]")),
			10_000,
		);
		await box.sendKeys("instal");
		await driver.wait(
			until.elementLocated(By.css("[role=listbox] [role=option]")),
			10_000,
		);
		await box.sendKeys(Key.ARROW_DOWN, Key.ENTER);
		await driver.wait(until.elementLocated(By.css("h1")), 10_000);
		await driver.wait(async () => {
			const h = await driver.findElement(By.css("h1")).getText();
			return /install ai-gent/i.test(h);
		}, 10_000);

		console.log(
			"e2e smoke ok: landing, autonomous flow, skill page, not-found, install, skip link, compatibility and keyboard search",
		);
	} finally {
		await driver.quit();
	}
}

main().catch((error) => {
	console.error(error);
	process.exit(1);
});
