import { Command, GitBranch, Plus, Settings, X } from "lucide-react";
import type { ReactNode } from "react";
import { useAgents } from "../../shared/agents";
import { t } from "../../shared/i18n";
import { api, type Space } from "../../shared/ipc";
import { IS_MAC } from "../../shared/platform";
import { useSession } from "../../shared/session";
import { IconButton } from "../../shared/ui/IconButton";
import { ResizeHandle } from "../../shared/ui/ResizeHandle";
import { Tooltip } from "../../shared/ui/Tooltip";
import { AgentsList } from "../agents/AgentsList";
import { openFolder, PALETTE_KEYS, togglePalette } from "../commands/commands";
import { openSettings } from "../settings/dialog";
import {
  DEFAULT_SIDEBAR_WIDTH,
  MIN_SIDEBAR_WIDTH,
  maxSidebarWidth,
  useSidebar,
} from "./sidebarWidth";

const HUES = [264, 160, 30, 330, 200, 95];

export function Sidebar() {
  const view = useSession((s) => s.view);
  const width = useSidebar((s) => s.width);
  const agentCount = useAgents((s) => s.list.length);
  return (
    <aside style={{ width }} className="relative flex shrink-0 flex-col border-line border-r">
      <ResizeHandle
        side="right"
        value={width}
        min={MIN_SIDEBAR_WIDTH}
        max={maxSidebarWidth}
        label={t("resizeSidebar")}
        onChange={(next) => useSidebar.setState({ width: next })}
        onReset={() => useSidebar.setState({ width: DEFAULT_SIDEBAR_WIDTH })}
      />
      <div
        data-tauri-drag-region
        className={`flex h-11 shrink-0 items-center gap-2 ${IS_MAC ? "pr-4 pl-20" : "px-4"}`}
      >
        <img src="/solace.svg" alt="" className="pointer-events-none size-5" />
        <span className="pointer-events-none font-semibold text-sm tracking-tight">Solace</span>
      </div>
      <SectionHeader title={t("spaces")}>
        <IconButton label={t("openFolder")} onClick={openFolder}>
          <Plus aria-hidden />
        </IconButton>
      </SectionHeader>
      <nav
        aria-label={t("spaces")}
        className="flex min-h-0 flex-1 flex-col gap-px overflow-y-auto px-2 pt-1 pb-2"
      >
        {view?.spaces.map((space) => (
          <SpaceRow
            key={space.id}
            space={space}
            active={space.id === view.activeSpace}
            branch={view.branches[space.id]}
          />
        ))}
        {view?.spaces.length === 0 && (
          <p className="px-2 text-fg-subtle text-xs">{t("noSpaces")}</p>
        )}
      </nav>
      <div className="flex min-h-0 flex-1 flex-col border-line border-t">
        <SectionHeader title={t("agents")} count={agentCount} />
        <div className="min-h-0 flex-1 overflow-y-auto pb-2">
          <AgentsList />
        </div>
      </div>
      <div className="flex items-center gap-1 border-line border-t p-2">
        <Tooltip label={t("settings")} shortcut="Ctrl+,">
          <button
            type="button"
            onClick={openSettings}
            className="flex h-8 flex-1 items-center gap-2.5 rounded-lg px-2 text-body text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
          >
            <Settings aria-hidden className="size-4" />
            {t("settings")}
          </button>
        </Tooltip>
        <IconButton label={t("commands")} shortcut={PALETTE_KEYS} onClick={togglePalette}>
          <Command aria-hidden />
        </IconButton>
      </div>
    </aside>
  );
}

function SectionHeader({
  title,
  count,
  children,
}: {
  title: string;
  count?: number;
  children?: ReactNode;
}) {
  return (
    <div className="flex h-9 items-center justify-between pr-2 pl-4">
      <h2 className="flex items-center gap-1.5 font-medium text-fg-muted text-xs">
        {title}
        {count !== undefined && count > 0 && (
          <span className="text-fg-subtle/70 tabular-nums">{count}</span>
        )}
      </h2>
      {children}
    </div>
  );
}

function SpaceRow({ space, active, branch }: { space: Space; active: boolean; branch?: string }) {
  return (
    <div
      className={`group relative flex h-9 items-center rounded-lg transition-colors duration-150 ${
        active ? "bg-selected font-medium text-fg" : "text-fg-muted hover:bg-hover hover:text-fg"
      }`}
    >
      {active && (
        <span aria-hidden className="absolute inset-y-2 left-0 w-0.5 rounded-full bg-primary" />
      )}
      <button
        type="button"
        title={space.root}
        aria-current={active ? "page" : undefined}
        onClick={() => void api.spaceActivate(space.id)}
        className="flex h-full min-w-0 flex-1 items-center gap-2.5 rounded-lg px-3 text-body"
      >
        <span
          aria-hidden
          className="size-1.5 shrink-0 rounded-full"
          style={{ background: `oklch(0.68 0.15 ${HUES[space.id % HUES.length]})` }}
        />
        <span className="truncate">{space.name}</span>
        {branch && (
          <span className="ml-auto flex max-w-28 shrink-0 items-center gap-1 text-fg-subtle text-xs group-hover:invisible">
            <GitBranch aria-hidden className="size-3 shrink-0" />
            <span className="truncate">{branch}</span>
          </span>
        )}
      </button>
      <IconButton
        label={t("closeSpace")}
        onClick={() => api.spaceClose(space.id)}
        className="absolute right-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
      >
        <X aria-hidden />
      </IconButton>
    </div>
  );
}
