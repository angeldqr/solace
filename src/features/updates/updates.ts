import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";

const CHECK_EVERY = 6 * 60 * 60 * 1000;

/** An update downloaded and waiting for the user to restart; never applied on its own,
 * since restarting stops every shell and agent. */
export const useUpdate = create<{ ready: Update | null }>(() => ({ ready: null }));

async function checkAndDownload() {
  if (useUpdate.getState().ready) return;
  try {
    const update = await check();
    if (!update) return;
    await update.download();
    useUpdate.setState({ ready: update });
  } catch (error) {
    // Offline or GitHub unreachable: try again next round.
    console.warn("update check failed", error);
  }
}

export function startUpdates() {
  // Dev builds would "update" into the installed release.
  if (import.meta.env.DEV) return;
  void checkAndDownload();
  window.setInterval(() => void checkAndDownload(), CHECK_EVERY);
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
