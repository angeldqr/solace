import { Tabs } from "@base-ui/react/tabs";
import { FolderTree, GitCompare, Maximize2, Minimize2, X } from "lucide-react";
import { t } from "../../shared/i18n";
import type { Space } from "../../shared/ipc";
import { IconButton } from "../../shared/ui/IconButton";
import { ResizeHandle } from "../../shared/ui/ResizeHandle";
import { useSidebar } from "../spaces/sidebarWidth";
import { ChangesView } from "./ChangesView";
import { FileTree } from "./FileTree";
import {
  DEFAULT_WIDTH,
  MIN_WIDTH,
  maxWidth,
  type PanelTab,
  TERMINAL_MIN_WIDTH,
  usePanel,
} from "./panel";
import { useChanges } from "./useChanges";

/** Always leaves the terminal at least TERMINAL_MIN px; the minimum width wins in a tiny window. */
function panelWidth(width: number, wide: boolean, sidebar: number) {
  const room = `(100vw - ${sidebar}px - ${TERMINAL_MIN_WIDTH}px)`;
  const wanted = wide ? `max(${width}px, min(70vw, ${room}))` : `${width}px`;
  return `max(${MIN_WIDTH}px, min(${wanted}, ${room}))`;
}

const tabClass =
  "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-body text-fg-muted transition-colors hover:text-fg data-[active]:bg-selected data-[active]:text-fg [&>svg]:size-4";

export function RightPanel({ space }: { space: Space }) {
  const open = usePanel((s) => s.open);
  return open ? <Panel space={space} /> : null;
}

/** Split from RightPanel so git is only queried while the panel is showing. */
function Panel({ space }: { space: Space }) {
  const { tab, width, wide } = usePanel();
  const sidebar = useSidebar((s) => s.width);
  const changes = useChanges(space.id);
  const count = changes.data?.length ?? 0;

  return (
    <aside
      aria-label={t(tab)}
      style={{ width: panelWidth(width, wide, sidebar) }}
      className="@container relative mb-2 mr-2 flex shrink-0 flex-col rounded-lg border border-line bg-pane"
    >
      <ResizeHandle
        side="left"
        value={width}
        min={MIN_WIDTH}
        max={maxWidth}
        label={t("resizePanel")}
        onChange={(next) => usePanel.setState({ width: next, wide: false })}
        onReset={() => usePanel.setState({ width: DEFAULT_WIDTH, wide: false })}
      />
      {/* Clips the content to the rounded corners; the handle above sits outside it. */}
      <Tabs.Root
        value={tab}
        onValueChange={(value) => usePanel.setState({ tab: value as PanelTab })}
        className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[inherit]"
      >
        <div className="flex h-11 shrink-0 items-center gap-2 border-line border-b px-2">
          <Tabs.List className="flex min-w-0 items-center gap-1">
            <Tabs.Tab value="files" aria-label={t("files")} className={tabClass}>
              <FolderTree aria-hidden />
              <span className="hidden @[22rem]:inline">{t("files")}</span>
            </Tabs.Tab>
            <Tabs.Tab value="changes" aria-label={t("changes")} className={tabClass}>
              <GitCompare aria-hidden />
              <span className="hidden @[22rem]:inline">{t("changes")}</span>
              {count > 0 && (
                <span className="rounded-full bg-hover px-1.5 text-2xs text-fg-muted tabular-nums">
                  {count}
                </span>
              )}
            </Tabs.Tab>
          </Tabs.List>
          <span className="hidden min-w-0 flex-1 truncate text-right text-fg-subtle text-xs @[22rem]:block">
            {space.name}
          </span>
          <div className="ml-auto flex shrink-0 items-center gap-2">
            <IconButton
              label={t(wide ? "restorePanel" : "widenPanel")}
              onClick={() => usePanel.setState({ wide: !wide })}
            >
              {wide ? <Minimize2 aria-hidden /> : <Maximize2 aria-hidden />}
            </IconButton>
            <IconButton label={t("closePanel")} onClick={() => usePanel.setState({ open: false })}>
              <X aria-hidden />
            </IconButton>
          </div>
        </div>
        <Tabs.Panel value="files" className="min-h-0 flex-1 overflow-auto text-body outline-none">
          {tab === "files" && <FileTree key={space.id} space={space.id} changes={changes} />}
        </Tabs.Panel>
        <Tabs.Panel value="changes" className="min-h-0 flex-1 overflow-auto text-body outline-none">
          {tab === "changes" && <ChangesView key={space.id} space={space.id} changes={changes} />}
        </Tabs.Panel>
      </Tabs.Root>
    </aside>
  );
}
