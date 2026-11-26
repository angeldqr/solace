import type { ITheme } from "@xterm/xterm";
import { DEFAULT_FAMILY, type Theme } from "./themes";

/** VS Code themes are JSON with comments and trailing commas; strip both, minding strings. */
export function parseJsonc(text: string): unknown {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      const start = i;
      for (i++; i < text.length && text[i] !== '"'; i++) if (text[i] === "\\") i++;
      out += text.slice(start, i + 1);
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      out += "\n";
    } else if (c === "/" && text[i + 1] === "*") {
      i = text.indexOf("*/", i + 2);
      if (i < 0) break;
      i++;
    } else {
      out += c;
    }
  }
  return JSON.parse(out.replace(/,(\s*[}\]])/g, "$1"));
}

/** Relative luminance (0 black – 1 white) of #rgb, #rrggbb or #rrggbbaa. */
export function hexLuminance(hex: string) {
  const digits = hex.slice(1);
  const full = digits.length <= 4 ? [...digits].map((d) => d + d).join("") : digits;
  const [r, g, b] = [0, 2, 4].map((i) => {
    const c = Number.parseInt(full.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

const ANSI: (keyof ITheme)[] = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "brightBlack",
  "brightRed",
  "brightGreen",
  "brightYellow",
  "brightBlue",
  "brightMagenta",
  "brightCyan",
  "brightWhite",
];

/** Builds a Solace theme from a VS Code colour theme file; missing colours come from the
 * matching built-in (dark or light). Throws with a readable message on bad input. */
export function fromVsCode(text: string): Theme {
  let json: { name?: string; type?: string; colors?: Record<string, string> };
  try {
    json = parseJsonc(text) as typeof json;
  } catch {
    throw new Error('not a valid VS Code theme (expected JSON with a "colors" object)');
  }
  const colors = json?.colors;
  if (!colors || typeof colors !== "object") {
    throw new Error('the theme has no "colors" object');
  }
  // Only hex colours, as VS Code writes them: nothing else ever reaches the page's styles.
  const pick = (...keys: string[]) =>
    keys.map((k) => colors[k]).find((v) => typeof v === "string" && /^#[0-9a-f]{3,8}$/i.test(v));
  const editor = pick("editor.background");
  // Theme files often leave the kind to their extension's manifest; then the background decides.
  const isDark = json.type
    ? json.type !== "light" && json.type !== "hc-light"
    : !editor || hexLuminance(editor) < 0.5;
  const base = isDark ? DEFAULT_FAMILY.dark : DEFAULT_FAMILY.light;
  const canvas = pick("sideBar.background", "activityBar.background") ?? editor;
  const accent = pick("button.background", "focusBorder", "activityBarBadge.background");

  const terminal: ITheme = { ...base.terminal };
  for (const name of ANSI) {
    const value = pick(`terminal.ansi${name[0].toUpperCase()}${name.slice(1)}`);
    if (value) (terminal as Record<string, string>)[name] = value;
  }
  terminal.background = pick("terminal.background") ?? editor ?? base.terminal.background;
  terminal.foreground =
    pick("terminal.foreground", "editor.foreground") ?? base.terminal.foreground;
  terminal.cursor =
    pick("terminalCursor.foreground", "editorCursor.foreground") ?? base.terminal.cursor;
  terminal.selectionBackground =
    pick("terminal.selectionBackground", "editor.selectionBackground") ??
    base.terminal.selectionBackground;

  return {
    id: "custom",
    name: typeof json.name === "string" && json.name ? json.name : "VS Code theme",
    dark: isDark,
    ui: {
      ...base.ui,
      canvas: canvas ?? base.ui.canvas,
      surface: pick("editorWidget.background", "dropdown.background") ?? base.ui.surface,
      pane: terminal.background ?? base.ui.pane,
      fg: pick("foreground", "editor.foreground") ?? base.ui.fg,
      "fg-muted": pick("descriptionForeground", "sideBar.foreground") ?? base.ui["fg-muted"],
      primary: accent ?? base.ui.primary,
      "primary-soft": accent ?? base.ui["primary-soft"],
    },
    terminal,
  };
}
