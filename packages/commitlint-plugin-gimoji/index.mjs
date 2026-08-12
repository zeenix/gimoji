import * as assert from "node:assert";

// The root path is the stable public location; the crate path is a fallback
// so the plugin keeps working from a checkout where only the crate copy
// exists (e.g. a PR that moves the file).
const EMOJIS_URLS = [
  "https://raw.githubusercontent.com/zeenix/gimoji/refs/heads/main/emojis.json",
  "https://raw.githubusercontent.com/zeenix/gimoji/refs/heads/main/crates/gimoji-core/emojis.json",
];

let gimojis;
let cause;
for (const url of EMOJIS_URLS) {
  try {
    const res = await fetch(url);
    assert.ok(res.ok);
    const json = await res.json();
    gimojis = json.gitmojis.map(({ emoji }) => emoji);
    break;
  } catch (e) {
    cause = e;
  }
}
if (gimojis === undefined) {
  throw new Error("unable to fetch gimoji emojis.json", { cause });
}

export default {
  helpUrl: "https://github.com/zeenix/gimoji/blob/main/CONTRIBUTING.md",
  rules: {
    "start-with-gimoji": ({ header = "" }) => {
      return [
        gimojis.some((symbol) => header.startsWith(symbol)),
        "commit message must begin with a Unicode emoji from https://zeenix.github.io/gimoji/",
      ];
    },
  },
};
