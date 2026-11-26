import type { ITheme } from "@xterm/xterm";

/** CSS colour tokens the UI is built from (`--color-<name>`, see index.css). */
export interface UiTokens {
  canvas: string;
  surface: string;
  pane: string;
  fg: string;
  "fg-muted": string;
  "fg-subtle": string;
  line: string;
  "line-strong": string;
  hover: string;
  selected: string;
  primary: string;
  "primary-soft": string;
}

export interface Theme {
  id: string;
  name: string;
  dark: boolean;
  ui: UiTokens;
  terminal: ITheme;
}

/** Dark ANSI colours shared by the dark presets; each sets its own background and accent. */
const DARK_ANSI = {
  black: "#27272a",
  red: "#f87171",
  green: "#4ade80",
  yellow: "#facc15",
  blue: "#818cf8",
  magenta: "#e879f9",
  cyan: "#22d3ee",
  white: "#d4d4d8",
  brightBlack: "#52525b",
  brightRed: "#fca5a5",
  brightGreen: "#86efac",
  brightYellow: "#fde047",
  brightBlue: "#a5b4fc",
  brightMagenta: "#f0abfc",
  brightCyan: "#67e8f9",
  brightWhite: "#fafafa",
};

/** Light ANSI colours shared by the light variants. */
const LIGHT_ANSI = {
  black: "#3f3f46",
  red: "#dc2626",
  green: "#16a34a",
  yellow: "#a16207",
  blue: "#4f46e5",
  magenta: "#c026d3",
  cyan: "#0e7490",
  white: "#a1a1aa",
  brightBlack: "#71717a",
  brightRed: "#ef4444",
  brightGreen: "#22c55e",
  brightYellow: "#ca8a04",
  brightBlue: "#6366f1",
  brightMagenta: "#d946ef",
  brightCyan: "#0891b2",
  brightWhite: "#d4d4d8",
};

/** A built-in theme: the user picks the family, the appearance picks the variant. */
export interface ThemeFamily {
  id: string;
  name: string;
  dark: Theme;
  light: Theme;
}

function dark(
  id: string,
  name: string,
  base: { canvas: string; surface: string; pane: string; primary: string; soft: string },
  terminal: Partial<ITheme> = {},
): Theme {
  return {
    id,
    name,
    dark: true,
    ui: {
      canvas: base.canvas,
      surface: base.surface,
      pane: base.pane,
      fg: "#f5f5f5",
      "fg-muted": "#a3a3a3",
      "fg-subtle": "#737373",
      line: "rgb(255 255 255 / 8%)",
      "line-strong": "rgb(255 255 255 / 16%)",
      hover: "rgb(255 255 255 / 6%)",
      selected: "rgb(255 255 255 / 8%)",
      primary: base.primary,
      "primary-soft": base.soft,
    },
    terminal: {
      ...DARK_ANSI,
      background: base.pane,
      foreground: "#e4e4e7",
      cursor: base.soft,
      cursorAccent: base.pane,
      selectionBackground: "rgb(255 255 255 / 18%)",
      ...terminal,
    },
  };
}

function light(
  id: string,
  name: string,
  base: { canvas: string; surface: string; pane: string; primary: string; soft: string },
  terminal: Partial<ITheme> = {},
): Theme {
  return {
    id,
    name,
    dark: false,
    ui: {
      canvas: base.canvas,
      surface: base.surface,
      pane: base.pane,
      fg: "#18181b",
      "fg-muted": "#52525b",
      "fg-subtle": "#8a8a93",
      line: "rgb(0 0 0 / 9%)",
      "line-strong": "rgb(0 0 0 / 22%)",
      hover: "rgb(0 0 0 / 4%)",
      selected: "rgb(0 0 0 / 7%)",
      primary: base.primary,
      "primary-soft": base.soft,
    },
    terminal: {
      ...LIGHT_ANSI,
      background: base.pane,
      foreground: "#27272a",
      cursor: base.primary,
      cursorAccent: base.pane,
      ...terminal,
    },
  };
}

export const FAMILIES: ThemeFamily[] = [
  {
    id: "solace",
    name: "Solace",
    dark: dark(
      "solace",
      "Solace",
      {
        canvas: "#000000",
        surface: "#0d0d0e",
        pane: "#0a0a0b",
        primary: "oklch(0.571 0.21 264)",
        soft: "oklch(0.82 0.08 264)",
      },
      { cursor: "#b4cbff", selectionBackground: "rgba(129, 140, 248, 0.3)" },
    ),
    light: light(
      "solace",
      "Solace",
      {
        canvas: "#f6f6f7",
        surface: "#ffffff",
        pane: "#ffffff",
        primary: "oklch(0.488 0.217 264)",
        soft: "oklch(0.55 0.18 264)",
      },
      { cursor: "#4f46e5", selectionBackground: "rgba(79, 70, 229, 0.18)" },
    ),
  },
  {
    id: "graphite",
    name: "Graphite",
    dark: dark("graphite", "Graphite", {
      canvas: "#151619",
      surface: "#1d1e22",
      pane: "#1a1b1f",
      primary: "oklch(0.62 0.15 245)",
      soft: "oklch(0.82 0.07 245)",
    }),
    light: light(
      "graphite",
      "Graphite",
      {
        canvas: "#f3f4f6",
        surface: "#ffffff",
        pane: "#fbfbfc",
        primary: "oklch(0.5 0.15 245)",
        soft: "oklch(0.56 0.13 245)",
      },
      { selectionBackground: "rgba(37, 99, 235, 0.18)" },
    ),
  },
  {
    id: "grove",
    name: "Grove",
    dark: dark(
      "grove",
      "Grove",
      {
        canvas: "#08100c",
        surface: "#0f1913",
        pane: "#0c1510",
        primary: "oklch(0.62 0.13 160)",
        soft: "oklch(0.84 0.09 160)",
      },
      { blue: "#7dd3fc", brightBlue: "#bae6fd" },
    ),
    light: light(
      "grove",
      "Grove",
      {
        canvas: "#f2f6f3",
        surface: "#ffffff",
        pane: "#fafcfa",
        primary: "oklch(0.5 0.12 160)",
        soft: "oklch(0.56 0.11 160)",
      },
      { selectionBackground: "rgba(5, 150, 105, 0.18)" },
    ),
  },
  {
    id: "ember",
    name: "Ember",
    dark: dark(
      "ember",
      "Ember",
      {
        canvas: "#100b09",
        surface: "#1a1310",
        pane: "#15100d",
        primary: "oklch(0.64 0.16 50)",
        soft: "oklch(0.84 0.09 60)",
      },
      { foreground: "#ede4df", blue: "#93c5fd", brightBlue: "#bfdbfe" },
    ),
    light: light(
      "ember",
      "Ember",
      {
        canvas: "#f8f4f1",
        surface: "#ffffff",
        pane: "#fdfbf9",
        primary: "oklch(0.55 0.15 50)",
        soft: "oklch(0.6 0.13 55)",
      },
      { selectionBackground: "rgba(234, 88, 12, 0.18)" },
    ),
  },
];

export const DEFAULT_FAMILY = FAMILIES[0];
