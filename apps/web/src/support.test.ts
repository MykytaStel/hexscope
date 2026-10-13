import { describe, expect, it } from "vitest";
import { WAYS, waysFor } from "./support";

describe("waysFor", () => {
  it("puts the way that suits the reader first and keeps every way", () => {
    expect(waysFor("en").map((w) => w.label)).toEqual(["Ko-fi", "Monobank jar"]);
    expect(waysFor("uk").map((w) => w.label)).toEqual(["Monobank jar", "Ko-fi"]);
    expect(waysFor("uk")).toHaveLength(WAYS.length);
  });

  it("links only to the payment pages themselves, over https", () => {
    for (const way of WAYS) expect(new URL(way.href).protocol).toBe("https:");
  });
});
