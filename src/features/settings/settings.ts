import { create } from "zustand";
import { DEFAULT_THEME, THEMES, type Theme } from "./themes";

export type CursorStyle = "block" | "bar" | "underline";
export type Language = "auto" | "en" | "es";

export interface Settings {
  theme: string;
  /** A theme imported from VS Code, selected with theme "custom". */
  custom: Theme | null;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  cursorStyle: CursorStyle;
  cursorBlink: boolean;
  language: Language;
}

/** Read by i18n.ts too, before anything else loads. */
export const SETTINGS_KEY = "solace.settings";

const DEFAULTS: Settings = {
  theme: DEFAULT_THEME.id,
  custom: null,
  fontFamily: "JetBrains Mono",
  fontSize: 13,
  lineHeight: 1.2,
  cursorStyle: "block",
  cursorBlink: true,
  language: "auto",
};

/** Stored settings, field by field: a bad value falls back to its default instead of breaking
 * startup (a terminal with line height < 1 can't even be created). */
function load(): Settings {
  let raw: Record<string, unknown>;
  try {
    raw = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}");
  } catch {
    return DEFAULTS;
  }
  const num = (v: unknown, min: number, max: number, fallback: number) =>
    typeof v === "number" && v >= min && v <= max ? v : fallback;
  const oneOf = <T extends string>(v: unknown, options: readonly T[], fallback: T) =>
    options.includes(v as T) ? (v as T) : fallback;
  return {
    theme: typeof raw.theme === "string" ? raw.theme : DEFAULTS.theme,
    custom: completeTheme(raw.custom),
    fontFamily:
      typeof raw.fontFamily === "string" && raw.fontFamily ? raw.fontFamily : DEFAULTS.fontFamily,
    fontSize: num(raw.fontSize, 9, 28, DEFAULTS.fontSize),
    lineHeight: num(raw.lineHeight, 1, 2, DEFAULTS.lineHeight),
    cursorStyle: oneOf(raw.cursorStyle, ["block", "bar", "underline"], DEFAULTS.cursorStyle),
    cursorBlink: typeof raw.cursorBlink === "boolean" ? raw.cursorBlink : DEFAULTS.cursorBlink,
    language: oneOf(raw.language, ["auto", "en", "es"], DEFAULTS.language),
  };
}

/** A stored imported theme over its built-in base, so tokens added later are never missing. */
function completeTheme(value: unknown): Theme | null {
  if (!value || typeof value !== "object") return null;
  const theme = value as Partial<Theme>;
  const base = (theme.dark === false ? THEMES.find((t) => !t.dark) : undefined) ?? DEFAULT_THEME;
  return {
    id: "custom",
    name: typeof theme.name === "string" ? theme.name : "VS Code theme",
    dark: theme.dark !== false,
    ui: { ...base.ui, ...(theme.ui ?? {}) },
    terminal: { ...base.terminal, ...(theme.terminal ?? {}) },
  };
}

export const useSettings = create<Settings>(load);

export function activeTheme(settings: Settings = useSettings.getState()): Theme {
  if (settings.theme === "custom" && settings.custom) return settings.custom;
  return THEMES.find((t) => t.id === settings.theme) ?? DEFAULT_THEME;
}

/** The bundled variable font has a different family name than the one users pick. */
export function terminalFontFamily(family: string) {
  const name = family === "JetBrains Mono" ? "JetBrains Mono Variable" : family;
  return `"${name}", "JetBrains Mono Variable", Consolas, monospace`;
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const [token, value] of Object.entries(theme.ui)) {
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
