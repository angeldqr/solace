import { Plus, SquareTerminal, X } from "lucide-react";
import { useAgents } from "../../shared/agents";
import { t } from "../../shared/i18n";
import { api, type Space } from "../../shared/ipc";
import { displayTitle, useSession } from "../../shared/session";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { IconButton } from "../../shared/ui/IconButton";

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
            className={`group flex h-7 max-w-52 items-center rounded-md transition-colors duration-150 ${
              active ? "bg-white/[0.08] text-fg" : "text-fg-muted hover:bg-hover hover:text-fg"
            }`}
          >
            <button
              type="button"
              role="tab"
              aria-selected={active}
              onClick={() => void api.tabActivate(tab.id)}
              className="flex h-full min-w-0 items-center gap-2 rounded-md pr-1 pl-2.5 text-xs"
            >
              {agent ? (
                <AgentIcon agent={agent.agent} />
              ) : (
                <SquareTerminal className="size-3.5 shrink-0" aria-hidden />
              )}
              <span className="truncate">
                {agent?.name ?? displayTitle(titles[tab.focused], t("terminal"))}
              </span>
            </button>
            <IconButton
              label={t("closeTab")}
              onClick={() => api.tabClose(tab.id)}
              className="mr-0.5 size-5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100 [&>svg]:size-3"
            >
              <X aria-hidden />
            </IconButton>
          </div>
        );
      })}
      <IconButton label={t("newTerminal")} onClick={() => api.tabNew(space.id)}>
        <Plus aria-hidden />
      </IconButton>
    </div>
  );
}
