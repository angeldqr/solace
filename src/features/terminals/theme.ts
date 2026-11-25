import type { ITheme } from "@xterm/xterm";

export const TERMINAL_BACKGROUND = "#0a0a0b";

/** ANSI palette tuned for the near-black pane surface; blue leans indigo to match the accent. */
export const terminalTheme: ITheme = {
  background: TERMINAL_BACKGROUND,
  foreground: "#e4e4e7",
  cursor: "#b4cbff",
  cursorAccent: TERMINAL_BACKGROUND,
  selectionBackground: "rgba(129, 140, 248, 0.3)",
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
