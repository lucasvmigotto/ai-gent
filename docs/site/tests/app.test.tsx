import { fireEvent, render, screen, within } from "@testing-library/react";
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

	it("skips to the content without leaving the page", () => {
		render(<App />);
		fireEvent.click(screen.getByRole("button", { name: /skip to content/i }));
		expect(document.activeElement).toBe(document.getElementById("main"));
		// Still the landing, not the not-found page "#main" would route to.
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

	it("switches the theme from the footer's icon buttons", async () => {
		render(<App />);
		const group = screen.getByRole("group", { name: "Theme" });
		const light = within(group).getByRole("button", { name: "Light theme" });
		const dark = within(group).getByRole("button", { name: "Dark theme" });
		expect(dark).toHaveAttribute("aria-pressed", "true");
		fireEvent.click(light);
		expect(light).toHaveAttribute("aria-pressed", "true");
		expect(document.documentElement.dataset.theme).toBe("light");
		fireEvent.click(dark);
		expect(document.documentElement.dataset.theme).toBe("dark");
		expect(screen.queryByRole("button", { name: "Toggle theme" })).toBeNull();
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

describe("search combobox", () => {
	function setup() {
		window.location.hash = "";
		render(<App />);
		return screen.getByRole("combobox");
	}

	it("is a labelled combobox that controls a listbox", () => {
		const box = setup();
		expect(box).toHaveAttribute("aria-expanded", "false");
		fireEvent.change(box, { target: { value: "commit" } });
		expect(box).toHaveAttribute("aria-expanded", "true");
		const list = screen.getByRole("listbox", { name: "Search results" });
		expect(box.getAttribute("aria-controls")).toBe(list.id);
		expect(within(list).getAllByRole("option").length).toBeGreaterThan(0);
	});

	it("announces the result count in a polite live region", () => {
		const box = setup();
		fireEvent.change(box, { target: { value: "commit" } });
		const live = document.querySelector("p[aria-live=polite]");
		expect(live?.textContent).toMatch(/^\d+ results$/);
	});

	it("moves with the arrows, opens with Enter, clears with Escape", () => {
		const box = setup();
		fireEvent.change(box, { target: { value: "instal" } });
		expect(box).not.toHaveAttribute("aria-activedescendant");
		fireEvent.keyDown(box, { key: "ArrowDown" });
		const first = box.getAttribute("aria-activedescendant");
		expect(first).toBeTruthy();
		expect(document.getElementById(first as string)).toHaveAttribute(
			"aria-selected",
			"true",
		);
		fireEvent.keyDown(box, { key: "ArrowDown" });
		expect(box.getAttribute("aria-activedescendant")).not.toBe(first);
		fireEvent.keyDown(box, { key: "ArrowUp" });
		expect(box.getAttribute("aria-activedescendant")).toBe(first);
		fireEvent.keyDown(box, { key: "Escape" });
		expect(box).toHaveValue("");
		expect(box).toHaveAttribute("aria-expanded", "false");
	});

	it("opens the Install page with Enter on a search for it", () => {
		const box = setup();
		fireEvent.change(box, { target: { value: "instal" } });
		fireEvent.keyDown(box, { key: "Enter" });
		expect(
			screen.getByRole("heading", { level: 1, name: "Install ai-gent" }),
		).toBeInTheDocument();
	});

	it("keeps / as the shortcut that focuses it", () => {
		const box = setup();
		fireEvent.keyDown(window, { key: "/" });
		expect(document.activeElement).toBe(box);
	});

	it("says so when nothing matches", () => {
		const box = setup();
		fireEvent.change(box, { target: { value: "zzzzqqqq" } });
		expect(screen.getByText(/Nothing matches/)).toBeInTheDocument();
		expect(box).toHaveAttribute("aria-expanded", "false");
	});
});
