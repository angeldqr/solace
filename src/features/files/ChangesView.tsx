import { useEffect, useRef, useState } from "react";
import { t } from "../../shared/i18n";
import type { Change, SpaceId } from "../../shared/ipc";
import { Tooltip } from "../../shared/ui/Tooltip";
import { DiffView } from "./DiffView";
import { FileIcon } from "./fileIcons";
import { usePanel } from "./panel";
import { type Changes, STATUS_COLOR, slash } from "./useChanges";

/** Below this the side-by-side columns don't fit, so opening a diff widens the panel. */
const NARROW = 560;

export function ChangesView({ space, changes: result }: { space: SpaceId; changes: Changes }) {
  const { data: changes, error } = result;
  const [selected, setSelected] = useState<Change | null>(null);
  /** The `wide` value to restore when the diff closes; null while no diff is open. */
  const restore = useRef<boolean | null>(null);

  const close = () => {
    if (restore.current !== null) usePanel.setState({ wide: restore.current });
    restore.current = null;
  };
  // Leaving the tab with a diff open must not leave the panel stretched.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `close` only reads a ref
  useEffect(() => close, []);

  if (selected) {
    return (
      <DiffView
        space={space}
        change={selected}
        onBack={() => {
          close();
          setSelected(null);
        }}
      />
    );
  }
  if (error) return <Notice text={error} />;
  if (changes === undefined) return null;
  if (changes === null) return <Notice text={t("notARepository")} />;
  if (changes.length === 0) return <Notice text={t("noChanges")} />;

  const open = (change: Change) => {
    const { wide, width } = usePanel.getState();
    restore.current = wide;
    if (width < NARROW) usePanel.setState({ wide: true });
    setSelected(change);
  };
  const added = changes.reduce((n, c) => n + (c.additions ?? 0), 0);
  const removed = changes.reduce((n, c) => n + (c.deletions ?? 0), 0);
  return (
    <div className="flex flex-col">
      <p className="flex h-9 items-center gap-2 px-3 text-fg-subtle text-xs">
        {t("changeCount", { n: changes.length })}
        <span className="ml-auto text-success tabular-nums">+{added}</span>
        <span className="text-danger tabular-nums">−{removed}</span>
      </p>
      {changes.map((change) => {
        const path = slash(change.path);
        const slashAt = path.lastIndexOf("/");
        const dir = slashAt === -1 ? "" : path.slice(0, slashAt);
        return (
          <Tooltip key={change.path} label={path}>
            <button
              type="button"
              onClick={() => open(change)}
              className="flex h-8 w-full items-center gap-2 px-3 text-left text-body text-fg-muted transition-colors hover:bg-hover hover:text-fg"
            >
              <FileIcon name={path.slice(slashAt + 1)} />
              <span className="flex min-w-0 flex-1 items-baseline gap-2 overflow-hidden">
                <span className="min-w-0 truncate font-medium text-fg">
                  {path.slice(slashAt + 1)}
                </span>
                {dir && (
                  <span className="min-w-0 shrink-[2] truncate text-fg-subtle text-xs">{dir}</span>
                )}
              </span>
              <span
                className={`w-4 shrink-0 text-center font-mono text-2xs ${STATUS_COLOR[change.status] ?? "text-fg-subtle"}`}
              >
                {change.status}
              </span>
              {change.additions !== null && (
                <span className="shrink-0 text-xs tabular-nums">
                  <span className="text-success">+{change.additions}</span>{" "}
                  <span className="text-danger">−{change.deletions ?? 0}</span>
                </span>
              )}
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
}

function Notice({ text }: { text: string }) {
  return <p className="px-3 py-3 text-body text-fg-subtle">{text}</p>;
}
