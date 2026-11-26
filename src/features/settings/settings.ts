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
}

/** Read by i18n.ts too, before anything else loads. */
export const SETTINGS_KEY = "solace.settings";

const DEFAULTS: Settings = {
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
};

/** Stored settings, field by field: a bad value falls back to its default instead of breaking
 * startup (a terminal with line height < 1 can't even be created). */
function load(): Settings {
  try {
    return parseSettings(JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}"));
  } catch {
    return DEFAULTS;
  }
}

export function parseSettings(value: unknown): Settings {
  const raw = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
  const num = (v: unknown, min: number, max: number, fallback: number) =>
    typeof v === "number" && v >= min && v <= max ? v : fallback;
  const oneOf = <T extends string>(v: unknown, options: readonly T[], fallback: T) =>
    options.includes(v as T) ? (v as T) : fallback;
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
    checkUpdates: typeof raw.checkUpdates === "boolean" ? raw.checkUpdates : DEFAULTS.checkUpdates,
  };
}

/** A stored imported theme over its built-in base, so tokens added later are never missing. */
function completeTheme(value: unknown): Theme | null {
  if (!value || typeof value !== "object") return null;
  const theme = value as Partial<Theme>;
  const base = theme.dark === false ? DEFAULT_FAMILY.light : DEFAULT_FAMILY.dark;
  return {
    id: "custom",
    name: typeof theme.name === "string" ? theme.name : "VS Code theme",
    dark: theme.dark !== false,
    ui: { ...base.ui, ...(theme.ui ?? {}) },
    terminal: { ...base.terminal, ...(theme.terminal ?? {}) },
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

/** The bundled variable font has a different family name than the one users pick. */
export function terminalFontFamily(family: string) {
  const name = family === "JetBrains Mono" ? "JetBrains Mono Variable" : family;
  return `"${name}", "JetBrains Mono Variable", Consolas, monospace`;
}

/** Applies the theme, contrast, UI scale and glass opacity to the page. */
export function applyAppearance(settings: Settings = useSettings.getState()) {
  const theme = activeTheme(settings);
  const root = document.documentElement;
  root.style.fontSize = `${settings.uiScale}px`;
  root.style.setProperty("--glass-opacity", `${settings.glassOpacity}%`);
  for (const [token, value] of Object.entries(adjustContrast(theme.ui, settings.contrast))) {
    root.style.setProperty(`--color-${token}`, value);
  }
  // Text on the accent colour: dark on a light accent (an imported yellow, say), else white.
  root.style.setProperty("--color-on-primary", isLight(theme.ui.primary) ? "#111113" : "#ffffff");
  root.style.colorScheme = theme.dark ? "dark" : "light";
  root.dataset.theme = theme.dark ? "dark" : "light";
}

/** Whether a CSS colour is light, measured by letting the browser paint it. */
function isLight(color: string) {
  const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  if (!ctx) return false;
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, 1, 1);
  const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
  return 0.299 * r + 0.587 * g + 0.114 * b > 160;
}

export function updateSettings(patch: Partial<Settings>) {
  useSettings.setState(patch);
}

useSettings.subscribe((settings) => {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
});
