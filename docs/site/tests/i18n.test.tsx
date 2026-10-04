import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { enUS, I18nProvider, ptBR, useI18n } from "../src/i18n";

function Probe() {
	const { t, locale, setLocale } = useI18n();
	return (
		<div>
			<span data-testid="locale">{locale}</span>
			<span data-testid="title">{t.landing.title}</span>
			<button type="button" onClick={() => setLocale("pt-BR")}>
				pt
			</button>
		</div>
	);
}

describe("i18n", () => {
	it("ships both locales with the same shape", () => {
		expect(Object.keys(enUS)).toEqual(Object.keys(ptBR));
		expect(ptBR.landing.title).not.toBe(enUS.landing.title);
	});

	it("defaults to en-US and switches locale", () => {
		render(
			<I18nProvider>
				<MemoryRouter>
					<Probe />
				</MemoryRouter>
			</I18nProvider>,
		);
		expect(screen.getByTestId("locale")).toHaveTextContent("en-US");
		expect(screen.getByTestId("title")).toHaveTextContent(enUS.landing.title);
		fireEvent.click(screen.getByRole("button", { name: "pt" }));
		expect(screen.getByTestId("title")).toHaveTextContent(ptBR.landing.title);
		expect(document.documentElement.lang).toBe("pt-BR");
	});
});
