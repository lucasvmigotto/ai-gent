import { HashRouter, Link, Route, Routes } from "react-router";
import { SITE_VERSION } from "./version";

function TopBar() {
	return (
		<header>
			<nav aria-label="Primary">
				<Link to="/">ai-gent docs</Link> <span>{SITE_VERSION}</span>
			</nav>
		</header>
	);
}

function Landing() {
	return (
		<main>
			<h1>Find the skill for the task.</h1>
			<p>36 skills · 9 plugins · en-US + pt-BR</p>
		</main>
	);
}

export function App() {
	return (
		<HashRouter>
			<TopBar />
			<Routes>
				<Route path="/" element={<Landing />} />
			</Routes>
		</HashRouter>
	);
}
