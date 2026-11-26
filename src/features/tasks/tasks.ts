import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { api, type BatchView } from "../../shared/ipc";

export const useTasks = create<{ batches: BatchView[] }>(() => ({ batches: [] }));

/** Subscribes to the backend's "tasks" event and loads what is already running. */
export async function startTasks() {
  let heard = false;
  await listen<BatchView[]>("tasks", (event) => {
    heard = true;
    useTasks.setState({ batches: event.payload });
  });
  const batches = await api.tasksGet();
  // An event that arrived while the snapshot was in flight is newer; keep it.
  if (!heard) useTasks.setState({ batches });
}
