import { open } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import type { MessageKey } from "../../shared/i18n";
import { api, type PaneId } from "../../shared/ipc";
import { matches } from "../../shared/keys";
import { current } from "../../shared/session";
import { focusTerminal } from "../terminals/registry";

export interface Command {
  id: string;
  title: MessageKey;
  keys?: string;
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
  { id: "next-tab", title: "nextTab", keys: "Ctrl+Tab", run: () => cycleTab(1) },
  { id: "previous-tab", title: "previousTab", keys: "Ctrl+Shift+Tab", run: () => cycleTab(-1) },
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
