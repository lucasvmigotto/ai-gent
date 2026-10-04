import { HashRouter, Route, Routes } from "react-router";
import { Layout } from "./components";
import { I18nProvider } from "./i18n";
import {
	ChangelogPage,
	ContributePage,
	InstallPage,
	LandingPage,
	NotFoundPage,
	ReferencePage,
	SkillPage,
	SkillsPage,
} from "./pages";

export function App() {
	return (
		<I18nProvider>
			<HashRouter>
				<Layout>
					<Routes>
						<Route path="/" element={<LandingPage />} />
						<Route path="/use/install" element={<InstallPage />} />
						<Route path="/use/skills" element={<SkillsPage />} />
						<Route path="/use/skills/:id" element={<SkillPage />} />
						<Route path="/contribute" element={<ContributePage />} />
						<Route path="/reference" element={<ReferencePage />} />
						<Route path="/changelog" element={<ChangelogPage />} />
						<Route path="*" element={<NotFoundPage />} />
					</Routes>
				</Layout>
			</HashRouter>
		</I18nProvider>
	);
}
