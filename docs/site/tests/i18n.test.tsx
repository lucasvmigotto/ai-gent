import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { I18nProvider, LOCALES, messages, useI18n } from "../src/i18n";

function Probe() {
	const { t, locale, setLocale } = useI18n();
	return (
		<div>
			<span data-testid="locale">{locale}</span>
			<span data-testid="title">{t.landing.title}</span>
			<button type="button" onClick={() => setLocale(LOCALES[0])}>
				set
			</button>
		</div>
	);
}

/** Every leaf key path of a messages object, so locales can be compared. */
function keyPaths(value: unknown, prefix = ""): string[] {
	if (value === null || typeof value !== "object") return [prefix];
	return Object.entries(value as Record<string, unknown>).flatMap(([k, v]) =>
		keyPaths(v, prefix ? `${prefix}.${k}` : k),
	);
}

describe("i18n", () => {
	it("has at least one locale and a complete message set for each", () => {
		expect(LOCALES.length).toBeGreaterThan(0);
		const reference = keyPaths(messages[LOCALES[0]]);
		for (const locale of LOCALES) {
			expect(messages[locale]).toBeDefined();
			expect(keyPaths(messages[locale])).toEqual(reference);
		}
	});

	it("defaults to the first locale and persists the choice", () => {
		localStorage.clear();
		render(
			<I18nProvider>
				<MemoryRouter>
					<Probe />
				</MemoryRouter>
			</I18nProvider>,
		);
		expect(screen.getByTestId("locale")).toHaveTextContent(LOCALES[0]);
		fireEvent.click(screen.getByRole("button", { name: "set" }));
		expect(localStorage.getItem("ai-gent-docs-locale")).toBe(LOCALES[0]);
		expect(document.documentElement.lang).toBe(LOCALES[0]);
	});
});
