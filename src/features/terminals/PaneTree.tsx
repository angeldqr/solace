import { Columns2, Rows2, SquareTerminal, X } from "lucide-react";
import { type PointerEvent, useEffect, useRef, useState } from "react";
import { t } from "../../shared/i18n";
import { api, type Layout, type PaneId, type Space, type Tab } from "../../shared/ipc";
import { displayTitle, useSession } from "../../shared/session";
import { IconButton } from "../../shared/ui/IconButton";
import { focusTerminal, mountTerminal } from "./registry";

interface NodeProps {
  space: Space;
  tab: Tab;
  layout: Layout;
  /** Child indexes from the tab's root split down to this node. */
  path: number[];
}

export function PaneTree({ space, tab }: { space: Space; tab: Tab }) {
  return <Node space={space} tab={tab} layout={tab.layout} path={[]} />;
}

function Node({ space, tab, layout, path }: NodeProps) {
  if (layout.type === "leaf") {
    return <TerminalPane space={space} pane={layout.pane} focused={tab.focused === layout.pane} />;
  }
  return <Split space={space} tab={tab} layout={layout} path={path} />;
}

function Split({
  space,
  tab,
  layout,
  path,
}: NodeProps & { layout: Extract<Layout, { type: "split" }> }) {
  const [ratio, setRatio] = useState(layout.ratio);
  const dragging = useRef(false);
  const container = useRef<HTMLDivElement>(null);
  const row = layout.direction === "row";

  useEffect(() => setRatio(layout.ratio), [layout.ratio]);

  const onPointerMove = (event: PointerEvent) => {
    const rect = container.current?.getBoundingClientRect();
    if (!dragging.current || !rect) return;
    const next = row
      ? (event.clientX - rect.left) / rect.width
      : (event.clientY - rect.top) / rect.height;
    setRatio(Math.min(0.9, Math.max(0.1, next)));
  };

  return (
    <div
      ref={container}
      className={`flex h-full min-h-0 w-full min-w-0 ${row ? "flex-row" : "flex-col"}`}
    >
      <div className="min-h-0 min-w-0 shrink-0" style={{ flexBasis: `${ratio * 100}%` }}>
        <Node space={space} tab={tab} layout={layout.first} path={[...path, 0]} />
      </div>
      {/* biome-ignore lint/a11y/useSemanticElements: an <hr> cannot hold the drag handle line */}
      <div
        role="separator"
        aria-orientation={row ? "vertical" : "horizontal"}
        className={`group relative shrink-0 ${row ? "w-1.5 cursor-col-resize" : "h-1.5 cursor-row-resize"}`}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId);
          dragging.current = true;
        }}
        onPointerMove={onPointerMove}
        onPointerUp={() => {
          dragging.current = false;
          void api.splitResize(tab.id, path, ratio);
        }}
      >
        <div
          className={`absolute bg-primary opacity-0 transition-opacity duration-150 group-hover:opacity-60 ${
            row ? "inset-y-2 left-1/2 w-px" : "inset-x-2 top-1/2 h-px"
          }`}
        />
      </div>
      <div className="min-h-0 min-w-0 flex-1">
        <Node space={space} tab={tab} layout={layout.second} path={[...path, 1]} />
      </div>
    </div>
  );
}

function TerminalPane({ space, pane, focused }: { space: Space; pane: PaneId; focused: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  const title = useSession((s) => s.titles[pane]);

  useEffect(() => {
    if (host.current) return mountTerminal(pane, host.current);
  }, [pane]);

  useEffect(() => {
    if (focused) focusTerminal(pane);
  }, [focused, pane]);

  return (
    <section
      aria-label={displayTitle(title, t("terminal"))}
      onPointerDownCapture={() => {
        if (!focused) void api.paneFocus(pane);
      }}
      className={`flex h-full min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border bg-pane transition-colors duration-150 ${
        focused ? "border-white/[0.16]" : "border-line"
      }`}
    >
      <header className="group flex h-7 shrink-0 items-center gap-2 border-line border-b pr-1 pl-2.5 text-2xs">
        <SquareTerminal className="size-3.5 shrink-0 text-fg-subtle" aria-hidden />
        <span className={focused ? "text-fg" : "text-fg-muted"}>
          {displayTitle(title, t("terminal"))}
        </span>
        <span className="truncate text-fg-subtle">{space.root}</span>
        <div className="ml-auto flex opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100">
          <IconButton label={t("splitRight")} onClick={() => api.paneSplit(pane, "row")}>
            <Columns2 aria-hidden />
          </IconButton>
          <IconButton label={t("splitDown")} onClick={() => api.paneSplit(pane, "column")}>
            <Rows2 aria-hidden />
          </IconButton>
          <IconButton label={t("closeTerminal")} onClick={() => api.paneClose(pane)}>
            <X aria-hidden />
          </IconButton>
        </div>
      </header>
      <div ref={host} className="min-h-0 flex-1 py-1.5 pl-2.5" />
    </section>
  );
}
