import { open } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import type { MessageKey } from "../../shared/i18n";
import { api, type Direction, type PaneId } from "../../shared/ipc";
import { matches } from "../../shared/keys";
import { current, splitAround } from "../../shared/session";
import { togglePanel } from "../files/panel";
import { openSettings } from "../settings/dialog";
import { focusTerminal } from "../terminals/registry";

export interface Command {
  id: string;
  title: MessageKey;
  keys?: string;
  /** Runs again while the keys are held. */
  repeat?: boolean;
  run: () => unknown;
}

export const PALETTE_KEYS = "Ctrl+Shift+P";

export const usePalette = create<{ open: boolean }>(() => ({ open: false }));

export function togglePalette() {
  const open = !usePalette.getState().open;
  usePalette.setState({ open });
  if (!open) focusTerminal(current().pane);
}

export async function openFolder() {
  const path = await open({ directory: true });
  if (typeof path === "string") await api.spaceOpen(path);
}

function withPane(fn: (pane: PaneId) => unknown) {
  const { pane } = current();
  return pane === undefined ? undefined : fn(pane);
}

function cycleTab(step: number) {
  const { space, tab } = current();
  if (!space || !tab) return;
  const index = space.tabs.indexOf(tab);
  const next = space.tabs[(index + step + space.tabs.length) % space.tabs.length];
  return api.tabActivate(next.id);
}

/** Moves the divider next to the focused pane; the backend keeps it within 10–90%. */
function moveDivider(direction: Direction, step: number) {
  const { tab, pane } = current();
  const split = tab && pane !== undefined && splitAround(tab.layout, pane, direction);
  return split && api.splitResize(tab.id, split.path, split.ratio + step);
}

const divider = (id: string, title: MessageKey, keys: string, direction: Direction, step: number) =>
  ({ id, title, keys, repeat: true, run: () => moveDivider(direction, step) }) satisfies Command;

export const commands: Command[] = [
  // Plain Ctrl+<letter> belongs to the shell and agents (Claude Code uses Ctrl+O), so app keys add Shift.
  { id: "open-folder", title: "openFolder", keys: "Ctrl+Shift+O", run: openFolder },
  {
    id: "new-terminal",
    title: "newTerminal",
    keys: "Ctrl+Shift+T",
    run: () => {
      const { space } = current();
      return space ? api.tabNew(space.id) : openFolder();
    },
  },
  {
    id: "split-right",
    title: "splitRight",
    keys: "Ctrl+Shift+D",
    run: () => withPane((pane) => api.paneSplit(pane, "row")),
  },
  {
    id: "split-down",
    title: "splitDown",
    keys: "Ctrl+Shift+E",
    run: () => withPane((pane) => api.paneSplit(pane, "column")),
  },
  {
    id: "close-terminal",
    title: "closeTerminal",
    keys: "Ctrl+Shift+W",
    run: () => withPane(api.paneClose),
  },
  divider("divider-left", "moveDividerLeft", "Alt+Shift+←", "row", -0.05),
  divider("divider-right", "moveDividerRight", "Alt+Shift+→", "row", 0.05),
  divider("divider-up", "moveDividerUp", "Alt+Shift+↑", "column", -0.05),
  divider("divider-down", "moveDividerDown", "Alt+Shift+↓", "column", 0.05),
  { id: "next-tab", title: "nextTab", keys: "Ctrl+Tab", run: () => cycleTab(1) },
  { id: "previous-tab", title: "previousTab", keys: "Ctrl+Shift+Tab", run: () => cycleTab(-1) },
  {
    id: "toggle-files",
    title: "toggleFiles",
    keys: "Ctrl+Shift+B",
    run: () => togglePanel("files"),
  },
  {
    id: "toggle-changes",
    title: "toggleChanges",
    keys: "Ctrl+Shift+G",
    run: () => togglePanel("changes"),
  },
  { id: "settings", title: "settings", keys: "Ctrl+,", run: openSettings },
  {
    id: "close-space",
    title: "closeSpace",
    run: () => {
      const { space } = current();
      return space && api.spaceClose(space.id);
    },
  },
];

export function runCommand(command: Command) {
  Promise.resolve(command.run()).catch((error) => console.error(command.id, error));
}

/** The command bound to a key event, if any. */
export function commandFor(event: KeyboardEvent) {
  return commands.find((c) => c.keys && matches(event, c.keys));
}

export function isAppShortcut(event: KeyboardEvent) {
  return matches(event, PALETTE_KEYS) || commandFor(event) !== undefined;
}
