import { describe, expect, it } from "vitest";
import { adjustContrast, fontStack, parseSettings, resolveTheme, textOn } from "./settings";
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

  it("shows Discord activity unless turned off, ignoring the old opt-in key", () => {
    expect(defaults.discordActivity).toBe(true);
    expect(parseSettings({ discordPresence: false }).discordActivity).toBe(true);
    expect(parseSettings({ discordActivity: "no" }).discordActivity).toBe(true);
    expect(parseSettings({ discordActivity: false }).discordActivity).toBe(false);
  });

  it("falls back on bad values", () => {
    const bad = parseSettings({ uiScale: 40, contrast: 5, glassOpacity: 0, theme: "nope" });
    expect(bad).toMatchObject({ uiScale: 16, contrast: 100, glassOpacity: 80, theme: "solace" });
  });
});

describe("personalization settings", () => {
  it("defaults", () => {
    expect(defaults).toMatchObject({
      uiFont: "Inter",
      codeFont: "JetBrains Mono",
      codeFontSize: 13,
      fontSmoothing: true,
      accent: null,
      customCss: "",
      motionMs: 150,
      wordWrap: true,
      diffLayout: "unified",
      diffColors: "red-green",
      diffIgnoreWhitespace: false,
    });
  });

  it("keeps valid values", () => {
    const ok = parseSettings({
      uiFont: "Segoe UI",
      codeFont: "Consolas",
      codeFontSize: 16,
      fontSmoothing: false,
      accent: "#10B981",
      customCss: "a{}",
      motionMs: 0,
      wordWrap: false,
      diffLayout: "split",
      diffColors: "blue-orange",
      diffIgnoreWhitespace: true,
    });
    expect(ok).toMatchObject({
      uiFont: "Segoe UI",
      codeFontSize: 16,
      accent: "#10B981",
      motionMs: 0,
    });
    expect(ok.diffLayout).toBe("split");
    expect(ok.diffIgnoreWhitespace).toBe(true);
  });

  it("falls back on invalid input, field by field", () => {
    const bad = parseSettings({
      uiFont: " ",
      codeFont: 3,
      codeFontSize: 40,
      fontSmoothing: "yes",
      accent: "red",
      motionMs: 999,
      wordWrap: 1,
      diffLayout: "wide",
      diffColors: "pink",
      diffIgnoreWhitespace: "no",
    });
    expect(bad).toMatchObject({
      uiFont: "Inter",
      codeFont: "JetBrains Mono",
      codeFontSize: 13,
      fontSmoothing: true,
      accent: null,
      motionMs: 150,
      wordWrap: true,
      diffLayout: "unified",
      diffColors: "red-green",
      diffIgnoreWhitespace: false,
    });
    expect(parseSettings({ accent: "#12345" }).accent).toBeNull();
  });

  it("truncates custom CSS to 100000 characters", () => {
    expect(parseSettings({ customCss: "x".repeat(100001) }).customCss).toHaveLength(100000);
  });
});

describe("legacy keys", () => {
  it("migrates the old diff layout and wrap keys when the setting is missing", () => {
    const migrated = parseSettings({}, { diffLayout: "split", wordWrap: false });
    expect(migrated).toMatchObject({ diffLayout: "split", wordWrap: false });
  });

  it("prefers the stored setting over a legacy key", () => {
    const stored = parseSettings(
      { diffLayout: "unified", wordWrap: true },
      { diffLayout: "split", wordWrap: false },
    );
    expect(stored).toMatchObject({ diffLayout: "unified", wordWrap: true });
  });
});

describe("textOn", () => {
  const rgb = (hex: string) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));

  it("picks dark text on light accents and white on dark ones", () => {
    expect(textOn(rgb("#10b981"))).toBe("#111113");
    expect(textOn(rgb("#06b6d4"))).toBe("#111113");
    expect(textOn(rgb("#6366f1"))).toBe("#ffffff");
  });
});

describe("fontStack", () => {
  it("maps the bundled fonts to their variable families", () => {
    expect(fontStack("Inter", "sans")).toMatch(/^"Inter Variable"/);
    expect(fontStack("JetBrains Mono", "mono")).toMatch(/^"JetBrains Mono Variable"/);
    expect(fontStack("System", "sans")).toBe("system-ui, sans-serif");
  });

  it("can't be broken out of by a typed name", () => {
    expect(fontStack('Foo"; color: red', "mono")).not.toMatch(/[;]|"Foo"; /);
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
