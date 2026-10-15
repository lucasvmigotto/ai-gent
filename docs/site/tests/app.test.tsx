import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../src/App";
import { sourceUrl, version, versions } from "../src/content";

// Guards the class of bug that blanked sibling docs sites: a HashRouter with a
// prefix basename renders nothing at "/". The app must mount and render its
// landing at the router root.
describe("App", () => {
	it("renders the landing heading at the router root", () => {
		render(<App />);
		expect(
			screen.getByRole("heading", { name: /find the skill for the task/i }),
		).toBeInTheDocument();
	});

	it("renders the site version", () => {
		render(<App />);
		expect(screen.getByTitle("ai-gent version")).toHaveTextContent(version);
	});

	it("shows the license as GPL v3+, linked to the LICENSE at the build's ref", () => {
		render(<App />);
		const license = screen.getByRole("link", { name: "GPL v3+" });
		expect(license).toHaveAttribute("href", sourceUrl("LICENSE"));
		expect(license).toHaveAttribute(
			"title",
			expect.stringContaining("GPL-3.0-or-later"),
		);
		expect(screen.queryByText("Source on GitHub")).toBeNull();
	});

	it("shows each version in the footer, linked to its tree", () => {
		render(<App />);
		const footer = screen.getByTitle("Versions");
		expect(footer.textContent).toBe(versions.map((v) => v.label).join(" · "));
		expect(versions.map((v) => v.name)).toEqual(["ai-gent", "docs"]);
		for (const v of versions.filter((x) => x.href)) {
			expect(screen.getByRole("link", { name: v.label })).toHaveAttribute(
				"href",
				v.href,
			);
		}
	});
});
