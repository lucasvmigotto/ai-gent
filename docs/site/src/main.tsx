import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./theme.css";

// Theme is dark-first; the stored choice wins (docs/product/ux-vision.md).
const storedTheme = localStorage.getItem("ai-gent-docs-theme");
document.documentElement.dataset.theme =
	storedTheme === "light" || storedTheme === "dark" ? storedTheme : "dark";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
