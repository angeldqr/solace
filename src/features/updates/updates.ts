import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";
import { useSettings } from "../settings/settings";

const CHECK_EVERY = 6 * 60 * 60 * 1000;

/** An update downloaded and waiting for the user to restart; never applied on its own,
 * since restarting stops every shell and agent. */
export const useUpdate = create<{ ready: Update | null }>(() => ({ ready: null }));

type CheckResult = "upToDate" | "ready" | "failed";
let running: Promise<CheckResult> | null = null;

/** Checks GitHub now and downloads a newer version, if any. A check already under way is
 * shared, so the button and the timer never download the same update twice. */
export function checkNow(): Promise<CheckResult> {
  running ??= checkOnce().finally(() => {
    running = null;
  });
  return running;
}

async function checkOnce(): Promise<CheckResult> {
  if (useUpdate.getState().ready) return "ready";
  try {
    const update = await check();
    if (!update) return "upToDate";
    await update.download();
    useUpdate.setState({ ready: update });
    return "ready";
  } catch (error) {
    // Offline or GitHub unreachable: the automatic check tries again next round.
    console.warn("update check failed", error);
    return "failed";
  }
}

function checkAndDownload() {
  if (useSettings.getState().checkUpdates) void checkNow();
}

export function startUpdates() {
  // Dev builds would "update" into the installed release.
  if (import.meta.env.DEV) return;
  checkAndDownload();
  window.setInterval(checkAndDownload, CHECK_EVERY);
}

export async function restartToUpdate() {
  const update = useUpdate.getState().ready;
  if (!update) return;
  try {
    // On Windows the installer takes over here and reopens Solace when done.
    await update.install();
    await relaunch();
  } catch (error) {
    // Drop the broken download; the next check fetches it again.
    useUpdate.setState({ ready: null });
    console.error("update install failed", error);
  }
}
