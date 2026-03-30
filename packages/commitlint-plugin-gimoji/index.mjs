import * as assert from "node:assert";

let gimojis;
try {
  const res = await fetch(
    "https://raw.githubusercontent.com/zeenix/gimoji/refs/heads/main/emojis.json",
  );
  assert.ok(res.ok);
  const json = await res.json();
  gimojis = json.gitmojis.map(({ emoji }) => emoji);
} catch (cause) {
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
