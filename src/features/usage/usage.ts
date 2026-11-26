import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { t } from "../../shared/i18n";
import type { Limit, UsageView } from "../../shared/ipc";

export const useUsage = create<{ view: UsageView | null }>(() => ({ view: null }));

let started = false;

/** Subscribes once to the backend's "usage" event; safe to call from every mount. */
export function startUsage() {
  if (started) return;
  started = true;
  let heard = false;
  void listen<UsageView>("usage", (event) => {
    heard = true;
    useUsage.setState({ view: event.payload });
  });
  void invoke<UsageView>("usage_get").then((view) => {
    // An event that arrived while the snapshot was in flight is newer; keep it.
    if (!heard) useUsage.setState({ view });
  });
}

/** A window whose reset time has passed: what was reported no longer holds. */
export function isStale(limit: Limit, now: number) {
  return limit.resetsAt !== null && limit.resetsAt * 1000 <= now;
}

/** Claude's 5-hour window, else its weekly one, else Codex's shortest; never a stale one. */
export function headline(view: UsageView | null, now: number) {
  const fresh = (limits: Limit[] | undefined, minutes?: number) =>
    limits?.find((l) => (minutes ? l.windowMinutes === minutes : true) && !isStale(l, now));
  const limit =
    fresh(view?.claude?.limits, 300) ??
    fresh(view?.claude?.limits, 10080) ??
    fresh(view?.codex?.limits); // sorted shortest first by the backend
  return limit ? limit.usedPercent : null;
}

/** Colour token by how close the limit is. */
export function tone(percent: number) {
  if (percent >= 85) return "text-danger";
  if (percent >= 60) return "text-warning";
  return "text-primary";
}

/** "2h 14m" / "2 h 14 min" by language; under a minute reads "<1m". */
export function duration(ms: number) {
  const m = Math.floor(Math.max(0, ms) / 60000);
  if (m < 1) return t("durLess");
  const d = Math.floor(m / 1440);
  if (d > 0) return t("durDH", { d, h: Math.floor((m % 1440) / 60) });
  const h = Math.floor(m / 60);
  return h > 0 ? t("durHM", { h, m: m % 60 }) : t("durM", { m });
}
