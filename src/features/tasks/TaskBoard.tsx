import { ChevronRight, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { type AgentStatus, statusOf, useAgents } from "../../shared/agents";
import { t } from "../../shared/i18n";
import type { BatchView, TaskView } from "../../shared/ipc";
import { api } from "../../shared/ipc";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { StatusDot } from "../../shared/ui/StatusDot";
import { Tooltip } from "../../shared/ui/Tooltip";
import { useTasks } from "./tasks";

const FINISHED = ["done", "failed", "closed"];

/** One group per `solace tasks start`, above the regular agent list. */
export function TaskBoard() {
  const batches = useTasks((s) => s.batches);
  if (batches.length === 0) return null;
  return (
    <div className="flex flex-col gap-2">
      {batches.map((batch) => (
        <BatchGroup key={batch.id} batch={batch} />
      ))}
    </div>
  );
}

function BatchGroup({ batch }: { batch: BatchView }) {
  const [error, setError] = useState<string | null>(null);
  const finished = batch.tasks.filter((task) => FINISHED.includes(task.status)).length;
  const total = batch.tasks.length;

  const allFinished = finished === total;
  // Removing worktrees takes two clicks: the second one confirms.
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const timer = setTimeout(() => setArmed(false), 4000);
    return () => clearTimeout(timer);
  }, [armed]);

  const cleanUp = () => {
    if (!armed) return setArmed(true);
    setArmed(false);
    setError(null);
    api.tasksClean(batch.id).catch((err) => setError(String(err)));
  };

  return (
    <section aria-label={t("tasksBatch", { batch: batch.id })}>
      <h3 className="flex h-7 items-center gap-1.5 px-2 text-fg-subtle text-xs">
        <span className="truncate">{t("tasksBatch", { batch: batch.id })}</span>
        <span className="text-fg-subtle/70 tabular-nums">
          {finished}/{total}
        </span>
        <Tooltip label={t("tasksCleanUpHint")}>
          <button
            type="button"
            onClick={cleanUp}
            disabled={!allFinished}
            className={`ml-auto flex h-6 items-center gap-1 rounded-md px-1.5 transition-colors hover:bg-hover disabled:pointer-events-none disabled:opacity-40 ${armed ? "text-danger" : "text-fg-subtle hover:text-fg"}`}
          >
            <Trash2 aria-hidden className="size-3" />
            {armed ? t("tasksConfirmCleanUp") : t("tasksCleanUp")}
          </button>
        </Tooltip>
      </h3>
      <div className="mx-2 mb-1 h-0.5 overflow-hidden rounded-full bg-hover">
        <div
          className="h-full rounded-full bg-primary transition-[width] duration-300"
          style={{ width: `${total === 0 ? 0 : (finished / total) * 100}%` }}
        />
      </div>
      {error && <p className="whitespace-pre-wrap px-2 pb-1 text-danger text-xs">{error}</p>}
      {batch.tasks.map((task) => (
        <TaskRow key={task.id} task={task} />
      ))}
    </section>
  );
}

function TaskRow({ task }: { task: TaskView }) {
  const [open, setOpen] = useState(false);
  const agent = useAgents((s) => s.list.find((a) => a.pane === task.pane));
  const done = useAgents((s) => s.done);
  const status = dotStatus(task, agent && statusOf(agent, done));
  const detail = task.summary ?? task.error;
  // shortcut: no "View changes" action; the Changes panel only diffs a space's own folder, so
  // showing a worktree needs backend support for another root.
  return (
    <div className="rounded-lg hover:bg-hover" title={detail ?? undefined}>
      <div className="flex min-h-9 items-center gap-2 px-2 py-1">
        <button
          type="button"
          disabled={task.pane === null}
          onClick={() => task.pane !== null && void api.paneFocus(task.pane)}
          className="flex min-w-0 flex-1 items-center gap-2.5 text-left text-body text-fg-muted transition-colors hover:text-fg disabled:cursor-default"
        >
          <AgentIcon agent={task.kind} className="size-4" />
          <span className="min-w-0">
            <span className="block truncate">{task.title}</span>
            {task.branch && (
              <span className="block truncate font-mono text-2xs text-fg-subtle">
                {task.branch}
              </span>
            )}
          </span>
        </button>
        {detail && (
          <button
            type="button"
            aria-expanded={open}
            aria-label={t(open ? "taskHideSummary" : "taskShowSummary")}
            onClick={() => setOpen(!open)}
            className="grid size-5 shrink-0 place-items-center rounded text-fg-subtle hover:text-fg"
          >
            <ChevronRight
              aria-hidden
              className={`size-3.5 transition-transform ${open ? "rotate-90" : ""}`}
            />
          </button>
        )}
        {task.status === "closed" && (
          <span className="shrink-0 text-2xs text-fg-subtle">{t("taskClosed")}</span>
        )}
        {status === "failed" ? (
          <span
            role="img"
            aria-label={t("taskFailed")}
            className="size-2 shrink-0 rounded-full bg-danger"
          />
        ) : (
          <StatusDot status={status} />
        )}
      </div>
      {open && detail && (
        <p className="line-clamp-4 whitespace-pre-wrap px-2 pb-2 pl-9 text-fg-muted text-xs">
          {detail}
        </p>
      )}
    </div>
  );
}

/** The live agent state while the worker runs; the task's own outcome once it has one. */
function dotStatus(task: TaskView, live: AgentStatus | undefined): AgentStatus | "failed" {
  switch (task.status) {
    case "done":
      return "done";
    case "failed":
      return "failed";
    case "closed":
      return "idle";
    default:
      return live ?? "working";
  }
}
