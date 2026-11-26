import { Tabs } from "@base-ui/react/tabs";
import { FolderTree, GitCompare, Maximize2, Minimize2, X } from "lucide-react";
import { t } from "../../shared/i18n";
import type { Space } from "../../shared/ipc";
import { IconButton } from "../../shared/ui/IconButton";
import { ResizeHandle } from "../../shared/ui/ResizeHandle";
import { ChangesView } from "./ChangesView";
import { FileTree } from "./FileTree";
import { DEFAULT_WIDTH, MIN_WIDTH, maxWidth, type PanelTab, usePanel } from "./panel";
import { useChanges } from "./useChanges";

const tabClass =
  "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-body text-fg-muted transition-colors hover:text-fg data-[active]:bg-selected data-[active]:text-fg [&>svg]:size-4";

export function RightPanel({ space }: { space: Space }) {
  const open = usePanel((s) => s.open);
  return open ? <Panel space={space} /> : null;
}

/** Split from RightPanel so git is only queried while the panel is showing. */
function Panel({ space }: { space: Space }) {
  const { tab, width, wide } = usePanel();
  const changes = useChanges(space.id);
  const count = changes.data?.length ?? 0;

  return (
    <aside
      aria-label={t(tab)}
      style={{ width: wide ? `max(${width}px, 70vw)` : width }}
      className="relative mb-2 mr-2 flex shrink-0 flex-col rounded-lg border border-line bg-pane"
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
          <Tabs.List className="flex items-center gap-1">
            <Tabs.Tab value="files" className={tabClass}>
              <FolderTree aria-hidden />
              {t("files")}
            </Tabs.Tab>
            <Tabs.Tab value="changes" className={tabClass}>
              <GitCompare aria-hidden />
              {t("changes")}
              {count > 0 && (
                <span className="rounded-full bg-hover px-1.5 text-2xs text-fg-muted tabular-nums">
                  {count}
                </span>
              )}
            </Tabs.Tab>
          </Tabs.List>
          <span className="ml-auto truncate text-fg-subtle text-xs">{space.name}</span>
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
