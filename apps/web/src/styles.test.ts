// The app's stylesheets, read in the order style.css imports them: a
// declaration that a later rule with the same selectors overrides, in the
// same media query, never applies. They piled up as the design changed —
// 134 were taken out in one go — and this keeps new ones from hiding there.
// (A fallback in the same rule, such as `height: 100vh; height: 100dvh`, is
// left alone: there the first one applies where the second is not known.)
import { readFileSync } from "node:fs";
import postcss from "postcss";
import { describe, expect, it } from "vitest";

const src = new URL("./", import.meta.url);
const files = [...readFileSync(new URL("style.css", src), "utf8").matchAll(/@import "\.\/([^"]+)"/g)].map((m) => m[1]);

describe("the app's stylesheets", () => {
  it("hold no declaration a later rule overrides", () => {
    const seen = new Map<string, { file: string; line: number; rule: postcss.Rule }>();
    const overridden: string[] = [];
    for (const file of files) {
      postcss.parse(readFileSync(new URL(file, src), "utf8")).walkRules((rule) => {
        if (rule.parent?.type === "atrule" && /keyframes/.test((rule.parent as postcss.AtRule).name)) return;
        const context: string[] = [];
        for (let p = rule.parent; p && p.type !== "root"; p = p.parent) {
          if (p.type === "atrule") context.unshift(`@${(p as postcss.AtRule).name} ${(p as postcss.AtRule).params}`);
        }
        const selectors = rule.selectors.map((s) => s.trim().replace(/\s+/g, " ")).sort().join(",");
        rule.walkDecls((d) => {
          const key = `${context.join(" ")}|${selectors}|${d.prop}|${d.important}`;
          const before = seen.get(key);
          if (before && before.rule !== rule) {
            overridden.push(`${before.file}:${before.line} ${selectors} { ${d.prop} } — by ${file}:${d.source?.start?.line}`);
          }
          seen.set(key, { file, line: d.source?.start?.line ?? 0, rule });
        });
      });
    }
    expect(overridden).toEqual([]);
  });
});
