import { useRef } from "react";
import { t } from "../../shared/i18n";
import type { Space } from "../../shared/ipc";
import { ChangesView } from "./ChangesView";
import { FileTree } from "./FileTree";
import { usePanel } from "./panel";

const MIN_WIDTH = 220;
const MAX_WIDTH = 720;

export function RightPanel({ space }: { space: Space }) {
  const { open, tab, width } = usePanel();
  const dragging = useRef(false);
  if (!open) return null;

  return (
    <aside
      aria-label={t(tab)}
      style={{ width }}
      className="relative mb-2 mr-2 flex shrink-0 flex-col overflow-hidden rounded-lg border border-line bg-pane"
    >
      {/* biome-ignore lint/a11y/useSemanticElements: an <hr> cannot be a drag handle */}
      <div
        role="separator"
        aria-orientation="vertical"
        aria-valuenow={width}
        tabIndex={-1}
        className="absolute inset-y-0 left-0 z-10 w-1.5 cursor-col-resize"
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          dragging.current = true;
        }}
        onPointerMove={(event) => {
          if (!dragging.current) return;
          const right = event.currentTarget.parentElement?.getBoundingClientRect().right ?? 0;
          const next = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, right - event.clientX));
          usePanel.setState({ width: next });
        }}
        onPointerUp={() => {
          dragging.current = false;
        }}
      />
      <div className="flex h-9 shrink-0 items-center gap-2 border-line border-b px-3">
        <h2 className="font-medium text-xs">{t(tab)}</h2>
        <span className="ml-auto truncate text-2xs text-fg-subtle">{space.name}</span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto text-xs">
        {tab === "files" ? (
          <FileTree key={space.id} space={space.id} />
        ) : (
          <ChangesView key={space.id} space={space.id} />
        )}
      </div>
    </aside>
  );
}
