import { describe, expect, it } from "vitest";
import { filterPages } from "./pages";

const words: Record<string, string> = {
  general: "General",
  appearance: "Apariencia",
  theme: "Tema",
  shortcuts: "Atajos",
  about: "Acerca de",
};
const translate = (key: string) => words[key] ?? "zzz";

describe("filterPages", () => {
  it("returns every page for an empty query", () => {
    expect(filterPages("", translate)).toHaveLength(7);
    expect(filterPages("   ", translate)).toHaveLength(7);
  });

  it("matches row keywords, ignoring case", () => {
    expect(filterPages("TEMA", translate)).toEqual(["appearance"]);
  });

  it("ignores accents on both sides", () => {
    expect(filterPages("apariencía", translate)).toEqual(["appearance"]);
    expect(filterPages("atajos", translate)).toEqual(["shortcuts"]);
    expect(filterPages("acerca", translate)).toEqual(["about"]);
  });

  it("returns nothing when no page matches", () => {
    expect(filterPages("qwerty", translate)).toEqual([]);
  });
});
