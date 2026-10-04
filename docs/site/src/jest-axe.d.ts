// Minimal typings for jest-axe (ships no bundled type entry point).
interface AxeViolation {
	id: string;
	impact?: string;
	description: string;
}

interface AxeResults {
	violations: AxeViolation[];
}

declare module "jest-axe" {
	export function axe(
		container: Element | Document | string,
		options?: Record<string, unknown>,
	): Promise<AxeResults>;
	export const toHaveNoViolations: {
		toHaveNoViolations(results: AxeResults): {
			pass: boolean;
			message(): string;
		};
	};
}
