import { Tabs } from "@base-ui/react/tabs";
import { FolderTree, GitCompare, Maximize2, Minimize2, X } from "lucide-react";
import { useState } from "react";
import { t } from "../../shared/i18n";
import type { Space } from "../../shared/ipc";
import { IconButton } from "../../shared/ui/IconButton";
import { ChangesView } from "./ChangesView";
import { FileTree } from "./FileTree";
import { DEFAULT_WIDTH, MIN_WIDTH, maxWidth, type PanelTab, usePanel } from "./panel";
import { useChanges } from "./useChanges";

const STEP = 24;
const clamp = (width: number) => Math.min(maxWidth(), Math.max(MIN_WIDTH, width));

const tabClass =
  "flex h-7 items-center gap-1.5 rounded-md px-2.5 text-body text-fg-muted transition-colors hover:text-fg data-[active]:bg-selected data-[active]:text-fg [&>svg]:size-4";

export function RightPanel({ space }: { space: Space }) {
  const open = usePanel((s) => s.open);
  return open ? <Panel space={space} /> : null;
}

/** Split from RightPanel so git is only queried while the panel is showing. */
function Panel({ space }: { space: Space }) {
  const { tab, width, wide } = usePanel();
  const [dragging, setDragging] = useState(false);
  const changes = useChanges(space.id);
  const count = changes.data?.length ?? 0;
  const stopDrag = () => {
    setDragging(false);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  };

  /** While wide, resizing starts from what is on screen, then drops wide mode. */
  const startResize = (handle: HTMLElement) => {
    if (!wide) return width;
    const shown = clamp(handle.parentElement?.getBoundingClientRect().width ?? width);
    usePanel.setState({ wide: false, width: shown });
    return shown;
  };

  return (
    <aside
      aria-label={t(tab)}
      style={{ width: wide ? `max(${width}px, 70vw)` : width }}
      className="relative mb-2 mr-2 flex shrink-0 flex-col rounded-lg border border-line bg-pane"
    >
      {/* biome-ignore lint/a11y/useSemanticElements: an <hr> cannot be a drag handle */}
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label={t("resizePanel")}
        aria-valuemin={MIN_WIDTH}
        aria-valuemax={maxWidth()}
        aria-valuenow={width}
        tabIndex={0}
        data-dragging={dragging || undefined}
        className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize outline-none after:absolute after:inset-y-0 after:left-1 after:w-px after:bg-transparent after:transition-colors hover:after:bg-primary focus-visible:after:bg-primary data-[dragging]:after:bg-primary"
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          startResize(event.currentTarget);
          setDragging(true);
          document.body.style.cursor = "col-resize";
          document.body.style.userSelect = "none";
        }}
        onPointerMove={(event) => {
          if (!dragging) return;
          const right = event.currentTarget.parentElement?.getBoundingClientRect().right ?? 0;
          usePanel.setState({ width: clamp(right - event.clientX) });
        }}
        onPointerUp={stopDrag}
        onLostPointerCapture={stopDrag}
        onDoubleClick={() => usePanel.setState({ width: DEFAULT_WIDTH, wide: false })}
        onKeyDown={(event) => {
          // The panel grows leftward, so ArrowLeft widens it.
          const delta = event.key === "ArrowLeft" ? STEP : event.key === "ArrowRight" ? -STEP : 0;
          if (!delta) return;
          event.preventDefault();
          usePanel.setState({ width: clamp(startResize(event.currentTarget) + delta) });
        }}
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
