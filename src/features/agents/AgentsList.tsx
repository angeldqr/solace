import { type AgentStatus, type AgentView, statusOf, useAgents } from "../../shared/agents";
import type { MessageKey } from "../../shared/i18n";
import { t } from "../../shared/i18n";
import { api, type SessionView } from "../../shared/ipc";
import { panesOf, useSession } from "../../shared/session";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { StatusDot } from "../../shared/ui/StatusDot";
import { formatElapsed, useNow } from "../../shared/useNow";

/** Most urgent first: what needs the user, then what just finished. */
const GROUPS: { status: AgentStatus; title: MessageKey }[] = [
  { status: "blocked", title: "needsAttention" },
  { status: "done", title: "agentDone" },
  { status: "working", title: "agentWorking" },
  { status: "idle", title: "agentIdle" },
];

export function AgentsList() {
  const { list, done } = useAgents();
  const view = useSession((s) => s.view);
  const now = useNow();

  if (list.length === 0) {
    return <p className="px-4 text-2xs text-fg-subtle">{t("agentsEmpty")}</p>;
  }
  return (
    <div className="flex flex-col gap-2 px-2">
      {GROUPS.map(({ status, title }) => {
        const agents = list.filter((a) => statusOf(a, done) === status);
        if (agents.length === 0) return null;
        return (
          <section key={status} aria-label={t(title)}>
            <h3 className="flex h-6 items-center gap-1.5 px-2 text-2xs text-fg-subtle">
              {t(title)}
              <span className="text-fg-subtle/70">{agents.length}</span>
            </h3>
            {agents.map((agent) => (
              <AgentRow
                key={agent.pane}
                agent={agent}
                status={status}
                space={spaceName(view, agent)}
                elapsed={formatElapsed(now - agent.since)}
              />
            ))}
          </section>
        );
      })}
    </div>
  );
}

function spaceName(view: SessionView | null, agent: AgentView) {
  return view?.spaces.find((s) => s.tabs.some((t) => panesOf(t.layout).includes(agent.pane)))?.name;
}

function AgentRow(props: {
  agent: AgentView;
  status: AgentStatus;
  space?: string;
  elapsed: string;
}) {
  return (
    <button
      type="button"
      onClick={() => void api.paneFocus(props.agent.pane)}
      className="flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-left text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
    >
      <AgentIcon agent={props.agent.agent} className="size-4" />
      <span className="truncate">{props.agent.label ?? props.agent.name}</span>
      {props.space && <span className="truncate text-2xs text-fg-subtle">{props.space}</span>}
      <span className="ml-auto shrink-0 text-2xs text-fg-subtle tabular-nums">{props.elapsed}</span>
      <StatusDot status={props.status} />
    </button>
  );
}
