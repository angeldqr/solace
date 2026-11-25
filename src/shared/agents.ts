import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { PaneId } from "./ipc";
import { current, useSession } from "./session";

export type AgentState = "idle" | "working" | "blocked";

export interface AgentView {
  pane: PaneId;
  /** Catalog id, e.g. "claude". */
  agent: string;
  name: string;
  state: AgentState;
  /** Unix ms of the last state change. */
  since: number;
}

interface AgentsState {
  list: AgentView[];
  /** Panes whose agent finished while the user was looking elsewhere. */
  done: Set<PaneId>;
}

export const useAgents = create<AgentsState>(() => ({ list: [], done: new Set() }));

function receive(next: AgentView[]) {
  const { list, done } = useAgents.getState();
  const watching = document.hasFocus() ? current().pane : undefined;
  const nextDone = new Set([...done].filter((pane) => next.some((a) => a.pane === pane)));
  for (const agent of next) {
    const before = list.find((a) => a.pane === agent.pane);
    if (agent.state !== "idle") nextDone.delete(agent.pane);
    // Only a real stint of work counts, not a brief redraw that read as working.
    else if (
      before?.state === "working" &&
      agent.since - before.since > 3000 &&
      agent.pane !== watching
    )
      nextDone.add(agent.pane);
  }
  useAgents.setState({ list: next, done: nextDone });
}

export async function startAgents() {
  let heard = false;
  await listen<AgentView[]>("agents", (event) => {
    heard = true;
    receive(event.payload);
  });
  const snapshot = await invoke<AgentView[]>("agents_get");
  // An event that arrived while the snapshot was in flight is newer; keep it.
  if (!heard) receive(snapshot);
}

/** Looking at a pane acknowledges its finished agent. */
function acknowledge() {
  const { pane } = current();
  const { done } = useAgents.getState();
  if (pane !== undefined && done.has(pane)) {
    const next = new Set(done);
    next.delete(pane);
    useAgents.setState({ done: next });
  }
}

useSession.subscribe(acknowledge);
window.addEventListener("focus", acknowledge);

export function agentFor(pane: PaneId) {
  return useAgents.getState().list.find((a) => a.pane === pane);
}

export function useAgent(pane: PaneId) {
  return useAgents((s) => s.list.find((a) => a.pane === pane));
}

/** What the UI shows: an idle agent that finished unseen reads as "done". */
export type AgentStatus = AgentState | "done";

export function statusOf(agent: AgentView, done: Set<PaneId>): AgentStatus {
  return agent.state === "idle" && done.has(agent.pane) ? "done" : agent.state;
}
