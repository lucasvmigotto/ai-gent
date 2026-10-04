declare const __AI_GENT_VERSION__: string;

/** Toolkit version, injected at build time (git tag, never hand-maintained). */
export const SITE_VERSION: string =
	typeof __AI_GENT_VERSION__ !== "undefined" ? __AI_GENT_VERSION__ : "dev";
