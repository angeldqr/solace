import { Menu } from "@base-ui/react/menu";
import { ChevronDown, Plus, SquareTerminal, X } from "lucide-react";
import { type KeyboardEvent, useState } from "react";
import { useAgents } from "../../shared/agents";
import { t } from "../../shared/i18n";
import { api, type Space } from "../../shared/ipc";
import { displayTitle, useSession } from "../../shared/session";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { IconButton } from "../../shared/ui/IconButton";
import { Kbd } from "../../shared/ui/Kbd";

const ITEM =
  "flex h-8 cursor-default select-none items-center gap-2 rounded-md px-2 text-body text-fg-muted outline-none transition-colors duration-150 data-disabled:opacity-50 data-highlighted:bg-hover data-highlighted:text-fg";

/** Chevron next to "+": a new tab with a plain terminal or with an installed agent already running. */
function NewTabMenu({ space }: { space: Space["id"] }) {
  const [installed, setInstalled] = useState<[string, string][]>([]);
  return (
    <Menu.Root onOpenChange={(open) => open && api.agentsInstalled().then(setInstalled)}>
      <Menu.Trigger
        aria-label={t("newTabWith")}
        className="flex size-6 items-center justify-center rounded-md text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
      >
        <ChevronDown className="size-3.5" aria-hidden />
      </Menu.Trigger>
      <Menu.Portal>
        <Menu.Positioner sideOffset={4} align="start" className="z-50">
          <Menu.Popup className="glass min-w-48 rounded-lg border border-line p-1 shadow-black/40 shadow-xl outline-none transition-[opacity,scale] duration-(--motion-duration) data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0">
            <Menu.Item className={ITEM} onClick={() => api.tabNew(space)}>
              <SquareTerminal className="size-4" aria-hidden />
              <span className="flex-1">{t("terminal")}</span>
              <Kbd keys="Ctrl+Shift+T" />
            </Menu.Item>
            <Menu.Separator className="my-1 h-px bg-line" />
            {installed.length === 0 && (
              <Menu.Item disabled className={ITEM}>
                {t("noAgentsInstalled")}
              </Menu.Item>
            )}
            {installed.map(([id, name]) => (
              <Menu.Item key={id} className={ITEM} onClick={() => api.tabNewAgent(space, id)}>
                <AgentIcon agent={id} className="size-4" />
                {name}
              </Menu.Item>
            ))}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

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
      <NewTabMenu space={space.id} />
    </div>
  );
}
