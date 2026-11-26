import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  api,
  type Direction,
  type Layout,
  type PaneId,
  type SessionView,
  type Space,
  type Tab,
} from "./ipc";

interface SessionState {
  view: SessionView | null;
  /** Titles terminals set via OSC escape sequences. */
  titles: Record<PaneId, string>;
}

export const useSession = create<SessionState>(() => ({ view: null, titles: {} }));

export async function startSession() {
  // Listen first so no update between the snapshot and the subscription is lost.
  await listen<SessionView>("session", (event) => useSession.setState({ view: event.payload }));
  const snapshot = await api.sessionGet();
  // An event that arrived while the snapshot was in flight is newer; keep it.
  useSession.setState((s) => (s.view ? {} : { view: snapshot }));
}

export function setTitle(pane: PaneId, title: string) {
  useSession.setState((s) => ({ titles: { ...s.titles, [pane]: title } }));
}

export function activeSpace(view: SessionView | null): Space | undefined {
  return view?.spaces.find((s) => s.id === view.activeSpace);
}

export function activeTab(space: Space | undefined): Tab | undefined {
  return space?.tabs.find((t) => t.id === space.activeTab);
}

/** The space, tab and pane keyboard commands act on. */
export function current() {
  const space = activeSpace(useSession.getState().view);
  const tab = activeTab(space);
  return { space, tab, pane: tab?.focused };
}

export function panesOf(layout: Layout): PaneId[] {
  return layout.type === "leaf"
    ? [layout.pane]
    : [...panesOf(layout.first), ...panesOf(layout.second)];
}

/** The split closest to `pane` that lays panes out along `direction`, and its path. */
export function splitAround(
  layout: Layout,
  pane: PaneId,
  direction: Direction,
  path: number[] = [],
): { path: number[]; ratio: number } | undefined {
  if (layout.type === "leaf") return undefined;
  const side = panesOf(layout.first).includes(pane) ? 0 : 1;
  if (side === 1 && !panesOf(layout.second).includes(pane)) return undefined;
  const inner = splitAround(side ? layout.second : layout.first, pane, direction, [...path, side]);
  return inner ?? (layout.direction === direction ? { path, ratio: layout.ratio } : undefined);
}

/** Shells often report their executable path as the title; show just the program name. */
export function displayTitle(title: string | undefined, fallback: string) {
  if (!title) return fallback;
  const name = title.split(/[\\/]/).pop() ?? title;
  return name.replace(/\.exe$/i, "") || fallback;
}
