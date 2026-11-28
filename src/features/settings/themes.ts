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
  /** Dark-only families keep their dark variant in light mode. */
  light?: Theme;
}

const ANSI_KEYS = [
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
] as const;

/** A published palette: its own text colours and all 16 ANSI colours (normal, then bright). */
interface Palette {
  canvas: string;
  surface: string;
  pane: string;
  fg: string;
  muted: string;
  subtle: string;
  primary: string;
  soft: string;
  ansi: string;
}

function fromPalette(id: string, name: string, isDark: boolean, p: Palette): Theme {
  const colors = p.ansi.split(/\s+/);
  if (colors.length !== 16) throw new Error(`${id}: 16 ANSI colours expected`);
  const ansi = Object.fromEntries(ANSI_KEYS.map((key, i) => [key, colors[i]]));
  const theme = (isDark ? dark : light)(id, name, p, { ...ansi, foreground: p.fg });
  theme.ui = { ...theme.ui, fg: p.fg, "fg-muted": p.muted, "fg-subtle": p.subtle };
  return theme;
}

function family(id: string, name: string, d: Palette, l?: Palette): ThemeFamily {
  return {
    id,
    name,
    dark: fromPalette(id, name, true, d),
    light: l && fromPalette(id, name, false, l),
  };
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

// Palettes from each theme's official source (all MIT; credits in NOTICE).
FAMILIES.push(
  family(
    "catppuccin",
    "Catppuccin",
    {
      canvas: "#11111b",
      surface: "#181825",
      pane: "#1e1e2e",
      fg: "#cdd6f4",
      muted: "#a6adc8",
      subtle: "#7f849c",
      primary: "#cba6f7",
      soft: "#b4befe",
      ansi: "#45475a #f38ba8 #a6e3a1 #f9e2af #89b4fa #f5c2e7 #94e2d5 #a6adc8 #585b70 #f37799 #89d88b #ebd391 #74a8fc #f2aede #6bd7ca #bac2de",
    },
    {
      canvas: "#dce0e8",
      surface: "#e6e9ef",
      pane: "#eff1f5",
      fg: "#4c4f69",
      muted: "#6c6f85",
      subtle: "#8c8fa1",
      primary: "#8839ef",
      soft: "#7287fd",
      ansi: "#5c5f77 #d20f39 #40a02b #df8e1d #1e66f5 #ea76cb #179299 #acb0be #6c6f85 #de293e #49af3d #eea02d #456eff #fe85d8 #2d9fa8 #bcc0cc",
    },
  ),
  family(
    "rose-pine",
    "Rosé Pine",
    {
      canvas: "#191724",
      surface: "#1f1d2e",
      pane: "#191724",
      fg: "#e0def4",
      muted: "#908caa",
      subtle: "#6e6a86",
      primary: "#c4a7e7",
      soft: "#ebbcba",
      ansi: "#26233a #eb6f92 #31748f #f6c177 #9ccfd8 #c4a7e7 #ebbcba #e0def4 #6e6a86 #eb6f92 #31748f #f6c177 #9ccfd8 #c4a7e7 #ebbcba #e0def4",
    },
    {
      canvas: "#f2e9e1",
      surface: "#fffaf3",
      pane: "#faf4ed",
      fg: "#575279",
      muted: "#797593",
      subtle: "#9893a5",
      primary: "#907aa9",
      soft: "#d7827e",
      ansi: "#f2e9e1 #b4637a #286983 #ea9d34 #56949f #907aa9 #d7827e #575279 #9893a5 #b4637a #286983 #ea9d34 #56949f #907aa9 #d7827e #575279",
    },
  ),
  family(
    "tokyo-night",
    "Tokyo Night",
    {
      canvas: "#16161e",
      surface: "#1e202e",
      pane: "#1a1b26",
      fg: "#c0caf5",
      muted: "#a9b1d6",
      subtle: "#565f89",
      primary: "#7aa2f7",
      soft: "#bb9af7",
      ansi: "#15161e #f7768e #9ece6a #e0af68 #7aa2f7 #bb9af7 #7dcfff #a9b1d6 #414868 #f7768e #9ece6a #e0af68 #7aa2f7 #bb9af7 #7dcfff #c0caf5",
    },
    {
      canvas: "#cbccd1",
      surface: "#dfe0e5",
      pane: "#d5d6db",
      fg: "#343b58",
      muted: "#4c505e",
      subtle: "#6c6e75",
      primary: "#34548a",
      soft: "#5a4a78",
      ansi: "#0f0f14 #8c4351 #33635c #8f5e15 #34548a #5a4a78 #0f4b6e #343b58 #9699a3 #8c4351 #33635c #8f5e15 #34548a #5a4a78 #0f4b6e #343b58",
    },
  ),
  family(
    "kanagawa",
    "Kanagawa",
    {
      canvas: "#16161d",
      surface: "#2a2a37",
      pane: "#1f1f28",
      fg: "#dcd7ba",
      muted: "#c8c093",
      subtle: "#727169",
      primary: "#7e9cd8",
      soft: "#957fb8",
      ansi: "#090618 #c34043 #76946a #c0a36e #7e9cd8 #957fb8 #6a9589 #c8c093 #727169 #e82424 #98bb6c #e6c384 #7fb4ca #938aa9 #7aa89f #dcd7ba",
    },
    {
      canvas: "#e7dba0",
      surface: "#f4eec4",
      pane: "#f2ecbc",
      fg: "#545464",
      muted: "#716e61",
      subtle: "#8a8980",
      primary: "#4d699b",
      soft: "#624c83",
      ansi: "#1f1f28 #c84053 #6f894e #77713f #4d699b #b35b79 #597b75 #545464 #8a8980 #d7474b #6e915f #836f4a #6693bf #624c83 #5e857a #43436c",
    },
  ),
  family(
    "everforest",
    "Everforest",
    {
      canvas: "#232a2e",
      surface: "#343f44",
      pane: "#2d353b",
      fg: "#d3c6aa",
      muted: "#9da9a0",
      subtle: "#7a8478",
      primary: "#a7c080",
      soft: "#83c092",
      ansi: "#475258 #e67e80 #a7c080 #dbbc7f #7fbbb3 #d699b6 #83c092 #d3c6aa #7a8478 #e67e80 #a7c080 #dbbc7f #7fbbb3 #d699b6 #83c092 #f2efdf",
    },
    {
      canvas: "#efebd4",
      surface: "#f4f0d9",
      pane: "#fdf6e3",
      fg: "#5c6a72",
      muted: "#829181",
      subtle: "#a6b0a0",
      primary: "#8da101",
      soft: "#35a77c",
      ansi: "#5c6a72 #f85552 #8da101 #dfa000 #3a94c5 #df69ba #35a77c #939f91 #829181 #f85552 #8da101 #dfa000 #3a94c5 #df69ba #35a77c #a6b0a0",
    },
  ),
  family(
    "flexoki",
    "Flexoki",
    {
      canvas: "#100f0f",
      surface: "#1c1b1a",
      pane: "#100f0f",
      fg: "#cecdc3",
      muted: "#878580",
      subtle: "#6f6e69",
      primary: "#da702c",
      soft: "#3aa99f",
      ansi: "#100f0f #d14d41 #879a39 #d0a215 #4385be #ce5d97 #3aa99f #878580 #575653 #af3029 #66800b #ad8301 #205ea6 #a02f6f #24837b #cecdc3",
    },
    {
      canvas: "#f2f0e5",
      surface: "#fffcf0",
      pane: "#fffcf0",
      fg: "#100f0f",
      muted: "#6f6e69",
      subtle: "#878580",
      primary: "#bc5215",
      soft: "#24837b",
      ansi: "#100f0f #af3029 #66800b #ad8301 #205ea6 #a02f6f #24837b #6f6e69 #b7b5ac #d14d41 #879a39 #d0a215 #4385be #ce5d97 #3aa99f #cecdc3",
    },
  ),
  family(
    "github",
    "GitHub",
    {
      canvas: "#010409",
      surface: "#151b23",
      pane: "#0d1117",
      fg: "#e6edf3",
      muted: "#9198a1",
      subtle: "#6e7681",
      primary: "#4493f8",
      soft: "#79c0ff",
      ansi: "#484f58 #ff7b72 #3fb950 #d29922 #58a6ff #bc8cff #39c5cf #b1bac4 #6e7681 #ffa198 #56d364 #e3b341 #79c0ff #d2a8ff #56d4dd #ffffff",
    },
    {
      canvas: "#f6f8fa",
      surface: "#ffffff",
      pane: "#ffffff",
      fg: "#1f2328",
      muted: "#59636e",
      subtle: "#818b98",
      primary: "#0969da",
      soft: "#218bff",
      ansi: "#24292f #cf222e #116329 #4d2d00 #0969da #8250df #1b7c83 #6e7781 #57606a #a40e26 #1a7f37 #633c01 #218bff #a475f9 #3192aa #8c959f",
    },
  ),
  family("nord", "Nord", {
    canvas: "#242933",
    surface: "#3b4252",
    pane: "#2e3440",
    fg: "#d8dee9",
    muted: "#aeb3bb",
    subtle: "#616e88",
    primary: "#88c0d0",
    soft: "#81a1c1",
    ansi: "#3b4252 #bf616a #a3be8c #ebcb8b #81a1c1 #b48ead #88c0d0 #e5e9f0 #4c566a #bf616a #a3be8c #ebcb8b #81a1c1 #b48ead #8fbcbb #eceff4",
  }),
  family("dracula", "Dracula", {
    canvas: "#191a21",
    surface: "#21222c",
    pane: "#282a36",
    fg: "#f8f8f2",
    muted: "#c2c3cc",
    subtle: "#6272a4",
    primary: "#bd93f9",
    soft: "#ff79c6",
    ansi: "#21222c #ff5555 #50fa7b #f1fa8c #bd93f9 #ff79c6 #8be9fd #f8f8f2 #6272a4 #ff6e6e #69ff94 #ffffa5 #d6acff #ff92df #a4ffff #ffffff",
  }),
);

/** Solace itself: always has both variants. */
export const DEFAULT_FAMILY = FAMILIES[0] as Required<ThemeFamily>;
