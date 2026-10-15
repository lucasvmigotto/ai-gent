import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../src/App";
import { version, versions } from "../src/content";

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
