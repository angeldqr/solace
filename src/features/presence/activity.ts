import type { AgentView } from "../../shared/agents";
import { t } from "../../shared/i18n";
import type { PresenceActivity } from "../../shared/ipc";

const RANK = { working: 0, blocked: 1, idle: 2 } as const;

/** The agent to show: the focused pane's, else the busiest one. */
export function leadAgent(list: AgentView[], focused: number | undefined): AgentView | undefined {
  return (
    list.find((a) => a.pane === focused) ??
    [...list].sort((a, b) => RANK[a.state] - RANK[b.state])[0]
  );
}

/** Only the agent's name and a count: never a folder, project, branch or file. */
export function activityFor(list: AgentView[], focused: number | undefined): PresenceActivity {
  const lead = leadAgent(list, focused);
  const working = list.filter((a) => a.state === "working").length;
  return {
    details: lead ? t("presenceCodingWith", { agent: lead.name }) : t("presenceInSolace"),
    state:
      working === 0
        ? undefined
        : working === 1
          ? t("presenceAgentWorking")
          : t("presenceAgentsWorking", { n: working }),
    smallImage: lead?.agent,
    smallText: lead?.name,
  };
}
