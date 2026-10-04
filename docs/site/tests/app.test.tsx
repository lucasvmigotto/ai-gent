import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../src/App";
import { version } from "../src/content";

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
});
