// Writes src/lib/locales/_source.en.json: every English UI string, keyed "namespace.key" (d, c, w, g).
// Translators (people or tools) copy it to <code>.json and translate the values; keys must stay the same.
// Run: node --experimental-strip-types --import ./scripts/register-hooks.mjs scripts/extract-strings.mjs
import { writeFileSync } from "node:fs";
import { D } from "../src/lib/i18n.ts";
import { C } from "../src/lib/chatStrings.ts";
import { W } from "../src/lib/wizard.ts";
import { GUIDE } from "../src/lib/guide.ts";

const out = {};
for (const [k, v] of Object.entries(D)) out["d." + k] = v.en;
for (const [k, v] of Object.entries(C)) out["c." + k] = v.en;
for (const [k, v] of Object.entries(W)) out["w." + k] = v.en;
for (const g of GUIDE) {
  out[`g.${g.id}.t`] = g.en.t;
  g.en.steps.forEach((s, i) => (out[`g.${g.id}.s${i}`] = s));
  if (g.en.note) out[`g.${g.id}.note`] = g.en.note;
}
writeFileSync(new URL("../src/lib/locales/_source.en.json", import.meta.url), JSON.stringify(out, null, 1) + "\n");
console.log(Object.keys(out).length, "strings");
