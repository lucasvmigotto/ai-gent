import { render, screen } from "@testing-library/react";
import { axe } from "jest-axe";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import { enUS, I18nProvider } from "../src/i18n";
import { LandingPage, SkillPage } from "../src/pages";

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

	it("renders a skill page with its invoke command", () => {
		renderSkill("qa-load");
		expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
			/load/i,
		);
		expect(screen.getByText(/\/qa:load/)).toBeInTheDocument();
	});

	it("has no accessibility violations on a skill page", async () => {
		const { container } = renderSkill("qa-load");
		await screen.findByText(/\/qa:load/);
		const results = await axe(container);
		expect(results.violations).toEqual([]);
	});
});
