import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { SpaceId } from "../../shared/ipc";

export type PanelTab = "files" | "changes";

interface PanelState {
  open: boolean;
  tab: PanelTab;
  width: number;
  /** Not persisted: the panel temporarily takes most of the window (reading a diff). */
  wide: boolean;
  /** Bumped when files in the active space change on disk, so open views refetch. */
  version: number;
}

export const DEFAULT_WIDTH = 420;
export const MIN_WIDTH = 280;
const ABSOLUTE_MAX_WIDTH = 960;
/** Evaluated at drag time: the panel never takes more than 60% of the window. */
export const maxWidth = () => Math.min(ABSOLUTE_MAX_WIDTH, Math.round(window.innerWidth * 0.6));

const STORAGE_KEY = "solace.panel";
const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Partial<PanelState>;

export const usePanel = create<PanelState>(() => ({
  open: saved.open ?? false,
  tab: saved.tab ?? "files",
  width:
    typeof saved.width === "number" && saved.width >= MIN_WIDTH && saved.width <= ABSOLUTE_MAX_WIDTH
      ? saved.width
      : DEFAULT_WIDTH,
  wide: false,
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
