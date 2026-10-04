import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

// Rendered page bodies already own the h1, so Markdown headings are demoted
// one level to keep one h1 per page.
const DEMOTED_HEADINGS = {
	h1: "h2",
	h2: "h3",
	h3: "h4",
	h4: "h5",
	h5: "h6",
	h6: "h6",
} as const;

// Loaded lazily (see components.tsx Markdown) so react-markdown stays out of
// the initial bundle and only downloads on skill and changelog pages.
export default function Markdown({ children }: { children: string }) {
	return (
		<ReactMarkdown remarkPlugins={[remarkGfm]} components={DEMOTED_HEADINGS}>
			{children}
		</ReactMarkdown>
	);
}
