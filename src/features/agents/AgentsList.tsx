import { type AgentStatus, type AgentView, statusOf, useAgents } from "../../shared/agents";
import type { MessageKey } from "../../shared/i18n";
import { t } from "../../shared/i18n";
import { api, type SessionView } from "../../shared/ipc";
import { panesOf, useSession } from "../../shared/session";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { StatusDot } from "../../shared/ui/StatusDot";
import { Tooltip } from "../../shared/ui/Tooltip";
import { formatElapsed, useNow } from "../../shared/useNow";
import { TaskBoard } from "../tasks/TaskBoard";
import { useTasks } from "../tasks/tasks";
import { Ring } from "../usage/UsageButton";
import { compact, useUsage } from "../usage/usage";

/** Most urgent first: what needs the user, then what just finished. */
const GROUPS: { status: AgentStatus; title: MessageKey }[] = [
  { status: "blocked", title: "needsAttention" },
  { status: "done", title: "agentDone" },
  { status: "working", title: "agentWorking" },
  { status: "idle", title: "agentIdle" },
];

export function AgentsList() {
  const { list: all, done } = useAgents();
  const batches = useTasks((s) => s.batches);
  const view = useSession((s) => s.view);
  const now = useNow();
  // Workers of a task batch are shown by the board, not twice.
  const taskPanes = new Set(batches.flatMap((b) => b.tasks.map((task) => task.pane)));
  const list = all.filter((a) => !taskPanes.has(a.pane));

  if (list.length === 0 && batches.length === 0) {
    return <p className="px-4 text-fg-subtle text-xs">{t("agentsEmpty")}</p>;
  }
  return (
    <div className="flex flex-col gap-2 px-2">
      <TaskBoard />
      {GROUPS.map(({ status, title }) => {
        const agents = list.filter((a) => statusOf(a, done) === status);
        if (agents.length === 0) return null;
        return (
          <section key={status} aria-label={t(title)}>
            <h3 className="flex h-7 items-center gap-1.5 px-2 text-fg-subtle text-xs">
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
  const ctx = useUsage((s) => s.view?.sessions.find((x) => x.pane === props.agent.pane));
  const context =
    ctx?.percent != null && ctx.limit !== null
      ? t("contextOf", { pct: Math.round(ctx.percent), limit: compact(ctx.limit) }) +
        (ctx.model ? ` · ${ctx.model}` : "")
      : undefined;
  return (
    <button
      type="button"
      aria-label={context && `${props.agent.label ?? props.agent.name} · ${context}`}
      onClick={() => void api.paneFocus(props.agent.pane)}
      className="flex h-9 w-full items-center gap-2.5 rounded-lg px-2 text-left text-body text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
    >
      <AgentIcon agent={props.agent.agent} className="size-4" />
      <span className="truncate">{props.agent.label ?? props.agent.name}</span>
      {props.space && <span className="truncate text-fg-subtle text-xs">{props.space}</span>}
      <span className="ml-auto shrink-0 text-fg-subtle text-xs tabular-nums">{props.elapsed}</span>
      {ctx?.percent != null && context && (
        <Tooltip label={context}>
          <span className="flex shrink-0 items-center gap-1 text-fg-subtle text-xs tabular-nums">
            <Ring percent={ctx.percent} />
            {Math.round(ctx.percent)}%
          </span>
        </Tooltip>
      )}
      {ctx && ctx.percent === null && (
        <Tooltip label={t("contextPending")}>
          <span className="shrink-0 text-fg-subtle text-xs">—</span>
        </Tooltip>
      )}
      <StatusDot status={props.status} />
    </button>
  );
}
