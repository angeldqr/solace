import { create } from "zustand";

export const DEFAULT_SIDEBAR_WIDTH = 256;
export const MIN_SIDEBAR_WIDTH = 208;
const ABSOLUTE_MAX_SIDEBAR_WIDTH = 400;
/** Resolved at drag time: the sidebar never takes more than 30% of the window. */
export const maxSidebarWidth = () =>
  Math.min(ABSOLUTE_MAX_SIDEBAR_WIDTH, Math.round(window.innerWidth * 0.3));

const STORAGE_KEY = "solace.sidebar";

function savedWidth() {
  try {
    const { width } = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (typeof width === "number" && width >= MIN_SIDEBAR_WIDTH && width <= maxSidebarWidth()) {
      return width;
    }
  } catch {
    // Unreadable value: fall back to the default.
  }
  return DEFAULT_SIDEBAR_WIDTH;
}

export const useSidebar = create<{ width: number }>(() => ({ width: savedWidth() }));

useSidebar.subscribe(({ width }) => localStorage.setItem(STORAGE_KEY, JSON.stringify({ width })));
