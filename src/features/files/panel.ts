import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { SpaceId } from "../../shared/ipc";

export type PanelTab = "files" | "changes";

interface PanelState {
  open: boolean;
  tab: PanelTab;
  width: number;
  /** Bumped when files in the active space change on disk, so open views refetch. */
  version: number;
}

const STORAGE_KEY = "solace.panel";
const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Partial<PanelState>;

export const usePanel = create<PanelState>(() => ({
  open: saved.open ?? false,
  tab: saved.tab ?? "files",
  width: saved.width ?? 300,
  version: 0,
}));

usePanel.subscribe(({ open, tab, width }) =>
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ open, tab, width })),
);

/** Opens the panel on `tab`, or closes it when that tab is already showing. */
export function togglePanel(tab: PanelTab) {
  const { open, tab: current } = usePanel.getState();
  usePanel.setState(open && current === tab ? { open: false } : { open: true, tab });
}

export async function startFileEvents(activeSpace: () => SpaceId | undefined) {
  await listen<SpaceId[]>("files-changed", (event) => {
    const space = activeSpace();
    if (space !== undefined && event.payload.includes(space)) {
      usePanel.setState((s) => ({ version: s.version + 1 }));
    }
  });
}
