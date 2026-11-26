import { Plus, Settings, X } from "lucide-react";
import type { ReactNode } from "react";
import { t } from "../../shared/i18n";
import { api, type Space } from "../../shared/ipc";
import { IS_MAC } from "../../shared/platform";
import { useSession } from "../../shared/session";
import { IconButton } from "../../shared/ui/IconButton";
import { AgentsList } from "../agents/AgentsList";
import { openFolder } from "../commands/commands";
import { openSettings } from "../settings/dialog";

const HUES = [264, 160, 30, 330, 200, 95];

export function Sidebar() {
  const view = useSession((s) => s.view);
  return (
    <aside className="flex w-[248px] shrink-0 flex-col border-line border-r">
      <div
        data-tauri-drag-region
        className={`flex h-11 shrink-0 items-center gap-2 ${IS_MAC ? "pr-4 pl-20" : "px-4"}`}
      >
        <img src="/solace.svg" alt="" className="pointer-events-none size-5" />
        <span className="pointer-events-none font-semibold text-[13px] tracking-tight">Solace</span>
      </div>
      <SectionHeader title={t("spaces")}>
        <IconButton label={t("openFolder")} onClick={openFolder}>
          <Plus aria-hidden />
        </IconButton>
      </SectionHeader>
      <nav aria-label={t("spaces")} className="flex flex-col gap-px px-2">
        {view?.spaces.map((space) => (
          <SpaceRow
            key={space.id}
            space={space}
            active={space.id === view.activeSpace}
            branch={view.branches[space.id]}
          />
        ))}
        {view?.spaces.length === 0 && (
          <p className="px-2 text-2xs text-fg-subtle">{t("noSpaces")}</p>
        )}
      </nav>
      <div className="mt-auto border-line border-t pb-2">
        <SectionHeader title={t("agents")} />
        <AgentsList />
      </div>
      <div className="border-line border-t p-2">
        <button
          type="button"
          onClick={openSettings}
          className="flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
        >
          <Settings aria-hidden className="size-4" />
          {t("settings")}
        </button>
      </div>
    </aside>
  );
}

function SectionHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="flex h-10 items-center justify-between pr-3 pl-4">
      <h2 className="font-medium text-2xs text-fg-muted">{title}</h2>
      {children}
    </div>
  );
}

function SpaceRow({ space, active, branch }: { space: Space; active: boolean; branch?: string }) {
  return (
    <div
      className={`group relative flex h-8 items-center rounded-md transition-colors duration-150 ${
        active ? "bg-selected text-fg" : "text-fg-muted hover:bg-hover hover:text-fg"
      }`}
    >
      <button
        type="button"
        title={space.root}
        aria-current={active ? "page" : undefined}
        onClick={() => void api.spaceActivate(space.id)}
        className="flex h-full min-w-0 flex-1 items-center gap-2.5 rounded-md px-2"
      >
        <span
          aria-hidden
          className="grid size-5 shrink-0 place-items-center rounded-[5px] font-semibold text-[11px] text-white"
          style={{ background: `oklch(0.52 0.13 ${HUES[space.id % HUES.length]})` }}
        >
          {space.name.charAt(0).toUpperCase()}
        </span>
        <span className="truncate">{space.name}</span>
        {branch && (
          <span className="ml-auto max-w-24 shrink-0 truncate text-2xs text-fg-subtle group-hover:invisible">
            {branch}
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
