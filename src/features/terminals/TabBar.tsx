import { Plus, SquareTerminal, X } from "lucide-react";
import type { KeyboardEvent } from "react";
import { useAgents } from "../../shared/agents";
import { t } from "../../shared/i18n";
import { api, type Space } from "../../shared/ipc";
import { displayTitle, useSession } from "../../shared/session";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { IconButton } from "../../shared/ui/IconButton";

/** Roving focus only: activation stays on click, Enter and Space. */
function moveFocus(event: KeyboardEvent<HTMLButtonElement>) {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
  const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
  if (!step) return;
  const tabs = [
    ...(event.currentTarget
      .closest('[role="tablist"]')
      ?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []),
  ];
  const next = tabs[tabs.indexOf(event.currentTarget) + step];
  if (!next) return;
  event.preventDefault();
  next.focus();
}

export function TabBar({ space }: { space: Space }) {
  const titles = useSession((s) => s.titles);
  const agents = useAgents((s) => s.list);
  return (
    <div role="tablist" className="flex h-full min-w-0 items-center gap-1 pl-2">
      {space.tabs.map((tab) => {
        const active = tab.id === space.activeTab;
        const agent = agents.find((a) => a.pane === tab.focused);
        return (
          <div
            key={tab.id}
            className={`group flex h-8 max-w-56 items-center rounded-lg transition-colors duration-150 ${
              active
                ? "bg-selected text-fg shadow-[inset_0_0_0_1px_var(--color-line)]"
                : "text-fg-muted hover:bg-hover hover:text-fg"
            }`}
          >
            <button
              type="button"
              role="tab"
              aria-selected={active}
              tabIndex={active ? 0 : -1}
              onClick={() => void api.tabActivate(tab.id)}
              onKeyDown={moveFocus}
              className="flex h-full min-w-0 items-center gap-2 rounded-lg pr-1 pl-2.5 text-body"
            >
              {agent ? (
                <AgentIcon agent={agent.agent} className="size-4" />
              ) : (
                <SquareTerminal className="size-4 shrink-0" aria-hidden />
              )}
              <span className="truncate">
                {agent?.label ?? agent?.name ?? displayTitle(titles[tab.focused], t("terminal"))}
              </span>
            </button>
            <IconButton
              label={t("closeTab")}
              onClick={() => api.tabClose(tab.id)}
              size="sm"
              className={`mr-1 ${
                active ? "" : "opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
              }`}
            >
              <X aria-hidden />
            </IconButton>
          </div>
        );
      })}
      <IconButton
        label={t("newTerminal")}
        shortcut="Ctrl+Shift+T"
        onClick={() => api.tabNew(space.id)}
      >
        <Plus aria-hidden />
      </IconButton>
    </div>
  );
}
