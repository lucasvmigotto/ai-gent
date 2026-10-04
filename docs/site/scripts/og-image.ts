// Renders the Open Graph image (1200×630) from scripts/og-image.svg to
// public/og-image.png, the exact filename index.html advertises.
//
//   bun run og
//
// Deterministic and browser-free (sharp/librsvg), like the sibling docs
// sites that render from an SVG source.
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const source = resolve(here, "og-image.svg");
const outDir = resolve(root, "public");

mkdirSync(outDir, { recursive: true });
await sharp(source)
	.resize(1200, 630)
	.png()
	.toFile(resolve(outDir, "og-image.png"));

console.log("public/og-image.png written (1200×630)");
