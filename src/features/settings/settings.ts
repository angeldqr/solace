import { create } from "zustand";
import { DEFAULT_FAMILY, FAMILIES, type Theme, type UiTokens } from "./themes";

export type CursorStyle = "block" | "bar" | "underline";
export type Language = "auto" | "en" | "es";
export type Appearance = "system" | "light" | "dark";

export interface Settings {
  /** A built-in family id, or "custom". */
  theme: string;
  appearance: Appearance;
  /** Root font size in px: scales every rem-based size. */
  uiScale: number;
  /** 50–200; 100 leaves the theme's text and border colours as they are. */
  contrast: number;
  /** Opacity (%) of glass surfaces such as menus and the palette. */
  glassOpacity: number;
  /** A theme imported from VS Code, selected with theme "custom". */
  custom: Theme | null;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  language: Language;
  /** Look for new versions on GitHub; the only request Solace makes on its own. */
  checkUpdates: boolean;
  /** UI font: "Inter", "Segoe UI", "System" or any installed family. */
  uiFont: string;
  /** Font for diffs, the file viewer and inline code. */
  codeFont: string;
  codeFontSize: number;
  fontSmoothing: boolean;
  /** #rrggbb overriding the theme's primary colour; null keeps the theme's. */
  accent: string | null;
  /** User stylesheet, applied on top of the theme. */
  customCss: string;
  /** Duration (ms) of popup and panel transitions; 0 turns them off. */
  motionMs: number;
  wordWrap: boolean;
  diffLayout: "unified" | "split";
  diffColors: "red-green" | "blue-orange";
  diffIgnoreWhitespace: boolean;
  /** Show the agent in use on Discord; opt-in. */
  discordPresence: boolean;
}

export const MAX_CSS = 100000;

/** Read by i18n.ts too, before anything else loads. */
export const SETTINGS_KEY = "solace.settings";

export const DEFAULTS: Settings = {
  theme: DEFAULT_FAMILY.id,
  appearance: "dark",
  uiScale: 16,
  contrast: 100,
  glassOpacity: 80,
  custom: null,
  fontFamily: "JetBrains Mono",
  fontSize: 13,
  lineHeight: 1.2,
  cursorStyle: "block",
  cursorBlink: true,
  language: "auto",
  checkUpdates: true,
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
  discordPresence: false,
};

/** Stored settings, field by field: a bad value falls back to its default instead of breaking
 * startup (a terminal with line height < 1 can't even be created). */
function load(): Settings {
  try {
    const legacy: Legacy = {};
    const style = localStorage.getItem("solace.diffStyle");
    const wrap = localStorage.getItem("solace.wrap");
    if (style) legacy.diffLayout = style === "split" ? "split" : "unified";
    if (wrap) legacy.wordWrap = wrap !== "off";
    const settings = parseSettings(JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}"), legacy);
    if (style || wrap) {
      // The old keys are gone, so the migrated values must be stored now, not on the next change.
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
      localStorage.removeItem("solace.diffStyle");
      localStorage.removeItem("solace.wrap");
    }
    return settings;
  } catch {
    return DEFAULTS;
  }
}

/** Settings that used to live in their own localStorage keys; used when the stored object lacks them. */
interface Legacy {
  diffLayout?: Settings["diffLayout"];
  wordWrap?: boolean;
}

export function parseSettings(value: unknown, legacy: Legacy = {}): Settings {
  const raw = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
  const num = (v: unknown, min: number, max: number, fallback: number) =>
    typeof v === "number" && v >= min && v <= max ? v : fallback;
  const oneOf = <T extends string>(v: unknown, options: readonly T[], fallback: T) =>
    options.includes(v as T) ? (v as T) : fallback;
  const bool = (v: unknown, fallback: boolean) => (typeof v === "boolean" ? v : fallback);
  const font = (v: unknown, fallback: string) => (typeof v === "string" && v.trim() ? v : fallback);
  const appearances = ["system", "light", "dark"] as const;
  // Before families there was a separate "daylight" theme.
  const daylight = raw.theme === "daylight";
  const known = typeof raw.theme === "string" && FAMILIES.some((f) => f.id === raw.theme);
  const custom = completeTheme(raw.custom);
  return {
    theme: daylight
      ? "solace"
      : (raw.theme === "custom" && custom) || known
        ? (raw.theme as string)
        : DEFAULTS.theme,
    appearance: oneOf(raw.appearance, appearances, daylight ? "light" : DEFAULTS.appearance),
    uiScale: num(raw.uiScale, 14, 18, DEFAULTS.uiScale),
    contrast: num(raw.contrast, 50, 200, DEFAULTS.contrast),
    glassOpacity: num(raw.glassOpacity, 40, 100, DEFAULTS.glassOpacity),
    custom,
    fontFamily:
      typeof raw.fontFamily === "string" && raw.fontFamily ? raw.fontFamily : DEFAULTS.fontFamily,
    fontSize: num(raw.fontSize, 9, 28, DEFAULTS.fontSize),
    lineHeight: num(raw.lineHeight, 1, 2, DEFAULTS.lineHeight),
    cursorStyle: oneOf(raw.cursorStyle, ["block", "bar", "underline"], DEFAULTS.cursorStyle),
    cursorBlink: typeof raw.cursorBlink === "boolean" ? raw.cursorBlink : DEFAULTS.cursorBlink,
    language: oneOf(raw.language, ["auto", "en", "es"], DEFAULTS.language),
    checkUpdates: bool(raw.checkUpdates, DEFAULTS.checkUpdates),
    uiFont: font(raw.uiFont, DEFAULTS.uiFont),
    codeFont: font(raw.codeFont, DEFAULTS.codeFont),
    codeFontSize: num(raw.codeFontSize, 10, 18, DEFAULTS.codeFontSize),
    fontSmoothing: bool(raw.fontSmoothing, DEFAULTS.fontSmoothing),
    accent: isHex(raw.accent) ? raw.accent : null,
    customCss: typeof raw.customCss === "string" ? raw.customCss.slice(0, MAX_CSS) : "",
    motionMs: num(raw.motionMs, 0, 400, DEFAULTS.motionMs),
    wordWrap: bool(raw.wordWrap, legacy.wordWrap ?? DEFAULTS.wordWrap),
    diffLayout: oneOf(
      raw.diffLayout,
      ["unified", "split"],
      legacy.diffLayout ?? DEFAULTS.diffLayout,
    ),
    diffColors: oneOf(raw.diffColors, ["red-green", "blue-orange"], DEFAULTS.diffColors),
    diffIgnoreWhitespace: bool(raw.diffIgnoreWhitespace, DEFAULTS.diffIgnoreWhitespace),
    discordPresence: bool(raw.discordPresence, DEFAULTS.discordPresence),
  };
}

/** #rrggbb: the only form the accent picker and the contrast check understand. */
export const isHex = (v: unknown): v is string =>
  typeof v === "string" && /^#[0-9a-f]{6}$/i.test(v);

/** A stored imported theme over its built-in base, so tokens added later are never missing. */
export function completeTheme(value: unknown): Theme | null {
  if (!value || typeof value !== "object") return null;
  const theme = value as Partial<Theme>;
  const base = theme.dark === false ? DEFAULT_FAMILY.light : DEFAULT_FAMILY.dark;
  // Only the known keys, and only strings: pasted JSON never adds anything else to the page.
  const merge = <T extends object>(from: T, over: unknown): T =>
    over && typeof over === "object"
      ? {
          ...from,
          ...Object.fromEntries(
            Object.entries(over).filter(
              ([k, v]) => Object.hasOwn(from, k) && typeof v === "string",
            ),
          ),
        }
      : from;
  return {
    id: "custom",
    name: typeof theme.name === "string" ? theme.name : "VS Code theme",
    dark: theme.dark !== false,
    ui: merge(base.ui, theme.ui),
    terminal: merge(base.terminal, theme.terminal),
  };
}

export const useSettings = create<Settings>(load);

const darkQuery =
  typeof window !== "undefined" && window.matchMedia
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null;

/** The OS colour scheme, for appearance "system". */
export const useSystemDark = create<boolean>(() => darkQuery?.matches ?? true);
darkQuery?.addEventListener("change", (event) => useSystemDark.setState(event.matches));

export function resolveTheme(settings: Settings, systemDark: boolean): Theme {
  // An imported theme has one variant; the appearance doesn't apply.
  if (settings.theme === "custom" && settings.custom) return settings.custom;
  const family = FAMILIES.find((f) => f.id === settings.theme) ?? DEFAULT_FAMILY;
  const isDark = settings.appearance === "system" ? systemDark : settings.appearance === "dark";
  return isDark ? family.dark : family.light;
}

export function activeTheme(settings: Settings = useSettings.getState()): Theme {
  return resolveTheme(settings, useSystemDark.getState());
}

/** Contrast above 100 pulls text and borders toward the foreground, below 100 toward the canvas. */
export function adjustContrast(ui: UiTokens, contrast: number): UiTokens {
  if (contrast === 100) return ui;
  const target = contrast > 100 ? ui.fg : ui.canvas;
  const pct = Math.round(Math.abs(contrast - 100) / 2);
  const mix = (value: string) => `color-mix(in oklab, ${value}, ${target} ${pct}%)`;
  return {
    ...ui,
    "fg-muted": mix(ui["fg-muted"]),
    "fg-subtle": mix(ui["fg-subtle"]),
    line: mix(ui.line),
    "line-strong": mix(ui["line-strong"]),
  };
}

/** A CSS font-family list for a family picked in settings. The bundled variable fonts have a
 * different family name than the one users pick; quotes and separators are dropped so a typed
 * name can't break out of the declaration. */
export function fontStack(family: string, kind: "sans" | "mono") {
  const name = family.replace(/["\\;{}]/g, "").trim();
  if (kind === "sans") {
    if (name === "System") return "system-ui, sans-serif";
    const first = name === "Inter" ? "Inter Variable" : name;
    return `"${first}", "Inter Variable", "Segoe UI", system-ui, sans-serif`;
  }
  const first = name === "JetBrains Mono" ? "JetBrains Mono Variable" : name;
  return `"${first}", "JetBrains Mono Variable", Consolas, monospace`;
}

export const terminalFontFamily = (family: string) => fontStack(family, "mono");

/** Applies the theme, contrast, UI scale and glass opacity to the page. */
export function applyAppearance(settings: Settings = useSettings.getState()) {
  const theme = activeTheme(settings);
  const root = document.documentElement;
  root.style.fontSize = `${settings.uiScale}px`;
  root.style.setProperty("--glass-opacity", `${settings.glassOpacity}%`);
  for (const [token, value] of Object.entries(adjustContrast(theme.ui, settings.contrast))) {
    root.style.setProperty(`--color-${token}`, value);
  }
  let primary = theme.ui.primary;
  if (settings.accent) {
    primary = settings.accent;
    // primary-soft is text on the surface: lighter than the accent on dark themes, darker on light.
    const soft = theme.dark
      ? `color-mix(in oklab, ${primary} 55%, white)`
      : `color-mix(in oklab, ${primary} 85%, black)`;
    root.style.setProperty("--color-primary", primary);
    root.style.setProperty("--color-primary-soft", soft);
  }
  root.style.setProperty("--color-on-primary", onPrimary(primary));
  root.style.setProperty("--font-sans", fontStack(settings.uiFont, "sans"));
  root.style.setProperty("--font-mono", fontStack(settings.codeFont, "mono"));
  root.style.setProperty("-webkit-font-smoothing", settings.fontSmoothing ? "antialiased" : "auto");
  root.style.setProperty("--motion-duration", `${settings.motionMs}ms`);
  const style = customStyle();
  const css = previewCss ?? settings.customCss;
  if (style.textContent !== css) style.textContent = css;
  root.style.colorScheme = theme.dark ? "dark" : "light";
  root.dataset.theme = theme.dark ? "dark" : "light";
}

/** The single <style> holding the user's CSS, created on first use. */
function customStyle() {
  let style = document.getElementById("solace-custom-css");
  if (!style) {
    style = document.createElement("style");
    style.id = "solace-custom-css";
    document.head.append(style);
  }
  return style;
}

/** CSS shown instead of the saved one while the user decides whether to keep it. */
let previewCss: string | null = null;

export function setPreviewCss(css: string | null) {
  previewCss = css;
  applyAppearance();
}

const relative = (c: number) => {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
};
const luminance = ([r, g, b]: readonly number[]) =>
  0.2126 * relative(r) + 0.7152 * relative(g) + 0.0722 * relative(b);

const DARK_TEXT = "#111113";
const DARK_TEXT_RGB = [0x11, 0x11, 0x13];

/** Whichever of white and near-black has the higher WCAG contrast ratio on an [r, g, b] background. */
export function textOn(rgb: readonly number[]) {
  const bg = luminance(rgb);
  const onWhite = 1.05 / (bg + 0.05);
  const onDark = (bg + 0.05) / (luminance(DARK_TEXT_RGB) + 0.05);
  return onDark > onWhite ? DARK_TEXT : "#ffffff";
}

const rgbCache = new Map<string, readonly number[]>();

/** Text colour for a background given as any CSS colour; the browser paints it to read it. */
function onPrimary(color: string) {
  let rgb = rgbCache.get(color);
  if (!rgb) {
    const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
    if (!ctx) return "#ffffff";
    ctx.fillStyle = color;
    ctx.fillRect(0, 0, 1, 1);
    rgb = Array.from(ctx.getImageData(0, 0, 1, 1).data.slice(0, 3));
    rgbCache.set(color, rgb);
  }
  return textOn(rgb);
}

export function updateSettings(patch: Partial<Settings>) {
  useSettings.setState(patch);
}

// Sliders fire per pixel: write once they settle, and on the way out.
let saveTimer: ReturnType<typeof setTimeout> | undefined;
const save = () => {
  saveTimer = undefined;
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(useSettings.getState()));
};

export function flushSettings() {
  if (saveTimer === undefined) return;
  clearTimeout(saveTimer);
  save();
}

useSettings.subscribe(() => {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(save, 250);
});
if (typeof window !== "undefined") {
  window.addEventListener("pagehide", flushSettings);
  window.addEventListener("beforeunload", flushSettings);
}
