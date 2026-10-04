import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./theme.css";

const storedTheme = localStorage.getItem("ai-gent-docs-theme");
const theme =
	storedTheme === "light" || storedTheme === "dark" ? storedTheme : "dark"; // dark-first per docs/product/ux-vision.md
document.documentElement.dataset.theme = theme;

const storedLocale = localStorage.getItem("ai-gent-docs-locale");
const locale = storedLocale === "pt-BR" ? "pt-BR" : "en-US"; // en-US default
document.documentElement.lang = locale;

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
