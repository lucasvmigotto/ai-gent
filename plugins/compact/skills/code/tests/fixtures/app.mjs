// ES module, semicolon style (prettier default).
import { readFileSync } from "node:fs";

const RE = /ab+c/gi;
const tpl = (strings, ...vals) => strings.raw.join("|") + vals.join(",");

export class Counter {
  #count = 0;
  static instances = 0;

  constructor(start = 0) {
    this.#count = start;
    Counter.instances++;
  }

  get value() {
    return this.#count;
  }

  *range(n) {
    for (let i = 0; i < n; i++) yield i;
  }

  async bump(by) {
    await null;
    this.#count += by ?? 1;
    return this;
  }
}

function divide(a, b) {
  return a / b / 2;
}

const obj = {
  a: 1,
  b: { c: [1, 2, 3] },
  "quoted key": `multi
  line   template ${1 + 1}`,
};

async function main() {
  const c = new Counter(5);
  await c.bump(2);
  const list = [3, 1, 2]
    .sort((x, y) => x - y)
    .map((x) => x * 2)
    .filter(Boolean);
  let i = 1;
  const j = i++ + ++i;
  const k = i - -j;
  const opt = obj?.b?.c?.[1] ?? "none";
  const tagged = tpl`a${1}b${2}c`;
  const isAbc = RE.test("xABBCx");
  if (list.length > 2) {
    console.log("long");
  } else {
    console.log("short");
  }
  try {
    JSON.parse("{bad");
  } catch (e) {
    console.log("caught", e.name);
  } finally {
    console.log("finally");
  }
  label: for (const x of [1, 2]) {
    if (x === 2) break label;
  }
  const re2 = /\/\*not a comment\*\//.source;
  console.log(c.value, [...c.range(3)], list, j, k, opt, tagged, isAbc, divide(8, 2), obj["quoted key"], re2, typeof readFileSync);
}

main();
