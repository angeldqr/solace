import { describe, expect, it } from "vitest";
import type { AgentView } from "../../shared/agents";
import { activityFor } from "./activity";

const agent = (pane: number, state: AgentView["state"], id = "claude"): AgentView => ({
  pane,
  agent: id,
  name: id === "claude" ? "Claude Code" : "Codex",
  label: null,
  state,
  since: 0,
});

describe("activityFor", () => {
  it("shows the terminal when no agent runs", () => {
    expect(activityFor([], undefined)).toMatchObject({ details: "In the terminal" });
  });

  it("prefers the focused pane's agent and counts the working ones", () => {
    const list = [agent(1, "working"), agent(2, "idle", "codex"), agent(3, "working")];
    expect(activityFor(list, 2)).toMatchObject({
      details: "Coding with Codex",
      state: "2 agents working",
      smallImage: "codex",
    });
  });

  it("falls back to the busiest agent and uses the singular", () => {
    const list = [agent(1, "idle", "codex"), agent(2, "working")];
    expect(activityFor(list, 9)).toMatchObject({
      details: "Coding with Claude Code",
      state: "1 agent working",
    });
  });
});
