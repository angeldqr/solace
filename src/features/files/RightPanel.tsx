import { Maximize2, Minimize2, X } from "lucide-react";
import { useEffect } from "react";
import { t } from "../../shared/i18n";
import type { Space } from "../../shared/ipc";
import { IconButton } from "../../shared/ui/IconButton";
import { ResizeHandle } from "../../shared/ui/ResizeHandle";
import { useSidebar } from "../spaces/sidebarWidth";
import { ChangesView } from "./ChangesView";
import { FilesTab } from "./FilesTab";
import { DEFAULT_WIDTH, MIN_WIDTH, maxWidth, TERMINAL_MIN_WIDTH, usePanel } from "./panel";
import { useChanges } from "./useChanges";

/** Always leaves the terminal at least TERMINAL_MIN px; the minimum width wins in a tiny window. */
function panelWidth(width: number, wide: boolean, sidebar: number) {
  const room = `(100vw - ${sidebar}px - ${TERMINAL_MIN_WIDTH}px)`;
  const wanted = wide ? `max(${width}px, min(70vw, ${room}))` : `${width}px`;
  return `max(${MIN_WIDTH}px, min(${wanted}, ${room}))`;
}

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
  useEffect(() => {
    usePanel.setState({ changes: count });
  }, [count]);
  useEffect(() => () => usePanel.setState({ changes: 0 }), []);

  return (
    <aside
      aria-labelledby="right-panel-title"
      style={{ width: panelWidth(width, wide, sidebar) }}
      className="@container starting:translate-x-2 starting:opacity-0 relative mb-2 mr-2 flex shrink-0 flex-col rounded-lg border border-line bg-pane transition-[opacity,translate] duration-(--motion-duration)"
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
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[inherit]">
        <div className="flex h-11 shrink-0 items-center gap-2 border-line border-b px-3">
          <h2 id="right-panel-title" className="min-w-0 truncate font-medium text-body text-fg">
            {t(tab)}
            <span className="hidden font-normal text-fg-subtle @[22rem]:inline">
              {" "}
              · {space.name}
            </span>
          </h2>
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
        <div
          className={`min-h-0 flex-1 text-body ${tab === "files" ? "overflow-hidden" : "overflow-auto"}`}
        >
          {tab === "files" ? (
            <FilesTab key={space.id} space={space.id} changes={changes} />
          ) : (
            <ChangesView key={space.id} space={space.id} changes={changes} />
          )}
        </div>
      </div>
    </aside>
  );
}
