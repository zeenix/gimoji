import assert from "node:assert";
import { describe, it } from "node:test";
import plugin from "./index.mjs";

describe("@gimoji/commitlint-plugin-gimoji", () => {
  describe("rules", () => {
    describe("start-with-gimoji", () => {
      for (const header of [undefined, "", "something"]) {
        it(`fails for "${header}"`, () => {
          const [got, _msg] = plugin.rules["start-with-gimoji"]({ header });
          assert.equal(got, false);
        });
      }

      for (const header of ["🎨something", "⚡️ something"]) {
        it(`passes for "${header}"`, () => {
          const [got, _msg] = plugin.rules["start-with-gimoji"]({ header });
          assert.equal(got, true);
        });
      }
    });
  });
});
