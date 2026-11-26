import { describe, expect, it } from "vitest";
import { adjustContrast, parseSettings, resolveTheme } from "./settings";
import { FAMILIES } from "./themes";

const defaults = parseSettings({});

describe("parseSettings", () => {
  it("fills the defaults", () => {
    expect(defaults).toMatchObject({
      theme: "solace",
      appearance: "dark",
      uiScale: 16,
      contrast: 100,
      glassOpacity: 80,
    });
  });

  it("migrates the old daylight theme", () => {
    expect(parseSettings({ theme: "daylight" })).toMatchObject({
      theme: "solace",
      appearance: "light",
    });
    expect(parseSettings({ theme: "daylight", appearance: "dark" }).appearance).toBe("dark");
  });

  it("falls back on bad values", () => {
    const bad = parseSettings({ uiScale: 40, contrast: 5, glassOpacity: 0, theme: "nope" });
    expect(bad).toMatchObject({ uiScale: 16, contrast: 100, glassOpacity: 80, theme: "solace" });
  });
});

describe("resolveTheme", () => {
  const pick = (appearance: "system" | "light" | "dark", systemDark: boolean) =>
    resolveTheme({ ...defaults, theme: "grove", appearance }, systemDark);

  it("picks the variant for the appearance", () => {
    expect(pick("light", true).dark).toBe(false);
    expect(pick("dark", false).dark).toBe(true);
    expect(pick("system", true).dark).toBe(true);
    expect(pick("system", false).dark).toBe(false);
  });

  it("lets an imported theme win over the appearance", () => {
    const custom = { ...FAMILIES[0].dark, id: "custom" };
    const theme = resolveTheme(
      { ...defaults, theme: "custom", custom, appearance: "light" },
      false,
    );
    expect(theme).toBe(custom);
  });
});

describe("adjustContrast", () => {
  const ui = FAMILIES[0].dark.ui;

  it("leaves 100 alone", () => {
    expect(adjustContrast(ui, 100)).toEqual(ui);
  });

  it("mixes toward the foreground above 100", () => {
    const out = adjustContrast(ui, 200);
    expect(out["fg-muted"]).toBe(`color-mix(in oklab, ${ui["fg-muted"]}, ${ui.fg} 50%)`);
    expect(out.fg).toBe(ui.fg);
    expect(out.canvas).toBe(ui.canvas);
    expect(out.primary).toBe(ui.primary);
  });

  it("mixes toward the canvas below 100", () => {
    expect(adjustContrast(ui, 50).line).toBe(`color-mix(in oklab, ${ui.line}, ${ui.canvas} 25%)`);
  });
});
