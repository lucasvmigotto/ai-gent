import { render, screen } from "@testing-library/react";
import { axe } from "jest-axe";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import { changelogVersions, skillById } from "../src/content";
import { enUS, I18nProvider } from "../src/i18n";
import {
	ChangelogPage,
	FlowPage,
	InstallPage,
	LandingPage,
	SkillPage,
} from "../src/pages";

function renderPage(node: React.ReactNode) {
	return render(
		<I18nProvider>
			<MemoryRouter>{node}</MemoryRouter>
		</I18nProvider>,
	);
}

function renderSkill(id: string) {
	return render(
		<I18nProvider>
			<MemoryRouter initialEntries={[`/use/skills/${id}`]}>
				<Routes>
					<Route path="/use/skills/:id" element={<SkillPage />} />
				</Routes>
			</MemoryRouter>
		</I18nProvider>,
	);
}

describe("pages", () => {
	it("shows the landing hero and the skill index", () => {
		renderPage(<LandingPage />);
		expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
			enUS.landing.title,
		);
		expect(
			screen.getByRole("heading", { name: enUS.landing.index }),
		).toBeInTheDocument();
	});

	it("has no accessibility violations on the landing page", async () => {
		const { container } = renderPage(<LandingPage />);
		const results = await axe(container);
		expect(results.violations).toEqual([]);
	});

	it("shows the install troubleshooting, one entry per problem", () => {
		renderPage(<InstallPage />);
		expect(
			screen.getByRole("heading", { name: enUS.install.troubleshooting }),
		).toBeInTheDocument();
		for (const item of enUS.install.troubleshootingItems) {
			expect(screen.getByText(item.problem)).toBeInTheDocument();
		}
	});

	it("renders the autonomous flow with every step and a link per skill", () => {
		renderPage(<FlowPage />);
		expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
			enUS.flow.title,
		);
		for (const s of enUS.flow.steps) {
			expect(
				screen.getByRole("heading", { level: 3, name: new RegExp(s.title) }),
			).toBeInTheDocument();
		}
		expect(document.body.textContent).toContain(enUS.flow.whenRed);
	});

	it("names only skills that exist on the flow page", () => {
		const ids = [
			...enUS.flow.steps.flatMap((s) => [
				...s.skills,
				...(s.track ?? []).flatMap((x) => x.skills),
			]),
			...enUS.flow.support.flatMap((x) => x.skills),
		];
		expect(ids.filter((id) => !skillById(id))).toEqual([]);
	});

	it("has no accessibility violations on the flow page", async () => {
		const { container } = renderPage(<FlowPage />);
		const results = await axe(container);
		expect(results.violations).toEqual([]);
	});

	it("renders a skill page with its invoke command", () => {
		renderSkill("qa-load");
		expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
			/load/i,
		);
		// Shown as Claude Code's command; the opencode id stays in the invoke block.
		expect(
			screen.getByRole("navigation", { name: "Breadcrumb" }),
		).toHaveTextContent("/qa:load");
		expect(document.body.textContent).toContain("/qa-load   # opencode");
	});

	it("has no accessibility violations on a skill page", async () => {
		const { container } = renderSkill("qa-load");
		await screen.findByRole("navigation", { name: "Breadcrumb" });
		const results = await axe(container);
		expect(results.violations).toEqual([]);
	});

	it("folds the changelog by major and minor, newest open", () => {
		renderPage(<ChangelogPage />);
		const released = changelogVersions.filter(
			(e) => e.version !== "Unreleased",
		);
		const majors = [
			...new Set(
				released.map((e) => e.major).filter((m): m is number => m !== null),
			),
		];
		const latest = Math.max(...majors);
		const group = screen.getByText(`${latest}.x`).closest("details");
		expect(group).toHaveAttribute("open");
		// an older major, if there is one, starts folded
		const older = majors.find((m) => m !== latest);
		if (older !== undefined) {
			const olderGroup = screen.getByText(`${older}.x`).closest("details");
			expect(olderGroup).not.toHaveAttribute("open");
		}
	});

	it("shows a released version's heading inside its folded groups", () => {
		renderPage(<ChangelogPage />);
		const released = changelogVersions.find((e) => e.version !== "Unreleased");
		if (!released) throw new Error("no released changelog entry to check");
		expect(
			screen.getByRole("heading", {
				level: 2,
				name: new RegExp(released.version.replace(/\./g, "\\.")),
			}),
		).toBeInTheDocument();
	});

	it("has no accessibility violations on the changelog page", async () => {
		const { container } = renderPage(<ChangelogPage />);
		const results = await axe(container);
		expect(results.violations).toEqual([]);
	});
});
