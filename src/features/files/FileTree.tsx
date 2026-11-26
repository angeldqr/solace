import { ChevronRight } from "lucide-react";
import { createContext, useContext, useLayoutEffect, useMemo, useRef, useState } from "react";
import { t } from "../../shared/i18n";
import { api, type FileEntry, type SpaceId } from "../../shared/ipc";
import { FileIcon, FolderIcon } from "./fileIcons";
import { type Changes, STATUS_COLOR, slash } from "./useChanges";
import { useOnDisk } from "./useOnDisk";

const INDENT = 14;
const BASE = 8;

/** Expanded folders per space, kept while the panel closes or the user switches spaces. */
const expanded = new Map<SpaceId, Set<string>>();

function expandedIn(space: SpaceId) {
  let set = expanded.get(space);
  if (!set) {
    set = new Set();
    expanded.set(space, set);
  }
  return set;
}

interface TreeState {
  selected: string | null;
  select: (path: string) => void;
  /** Git status letter per changed file path. */
  status: Map<string, string>;
  /** Folders that contain at least one changed file. */
  dirty: Set<string>;
}
const TreeContext = createContext<TreeState>({
  selected: null,
  select: () => {},
  status: new Map(),
  dirty: new Set(),
});

/** Arrow-key navigation over the rendered rows, so collapsed folders are skipped for free. */
function onTreeKeyDown(event: React.KeyboardEvent<HTMLElement>) {
  const row = (event.target as Element).closest<HTMLElement>("[role=treeitem]");
  if (!row) return;
  const rows = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("[role=treeitem]"));
  const at = rows.indexOf(row);
  const depth = Number(row.dataset.depth);
  const isFolder = row.getAttribute("aria-expanded") !== null;
  const open = row.getAttribute("aria-expanded") === "true";
  let target: HTMLElement | undefined;
  switch (event.key) {
    case "ArrowDown":
      target = rows[at + 1];
      break;
    case "ArrowUp":
      target = rows[at - 1];
      break;
    case "ArrowRight":
      if (!isFolder) return;
      if (!open) row.click();
      else if (Number(rows[at + 1]?.dataset.depth) > depth) target = rows[at + 1];
      break;
    case "ArrowLeft":
      if (isFolder && open) row.click();
      else
        target = rows
          .slice(0, at)
          .reverse()
          .find((r) => Number(r.dataset.depth) < depth);
      break;
    default:
      return;
  }
  event.preventDefault();
  target?.focus();
}

export function FileTree({ space, changes }: { space: SpaceId; changes: Changes }) {
  const [selected, select] = useState<string | null>(null);
  const tree = useRef<HTMLDivElement>(null);
  const git = useMemo(() => {
    const status = new Map<string, string>();
    const dirty = new Set<string>();
    for (const change of changes.data ?? []) {
      const path = slash(change.path);
      status.set(path, change.status);
      for (let i = path.indexOf("/"); i !== -1; i = path.indexOf("/", i + 1)) {
        dirty.add(path.slice(0, i));
      }
    }
    return { status, dirty };
  }, [changes.data]);
  const state = useMemo<TreeState>(() => ({ selected, select, ...git }), [selected, git]);

  // Collapsing, or a refresh that removes the selected row, would leave the tree without a tab stop.
  useLayoutEffect(() => {
    const el = tree.current;
    if (!el || selected === null) return;
    const check = () => {
      if (!el.querySelector('[role=treeitem][tabindex="0"]')) select(null);
    };
    check();
    const observer = new MutationObserver(check);
    observer.observe(el, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [selected]);

  return (
    <TreeContext.Provider value={state}>
      {/* The container is the tab stop until a row is selected; focus then moves to the first row. */}
      <div
        ref={tree}
        role="tree"
        aria-label={t("files")}
        tabIndex={selected === null ? 0 : -1}
        onFocus={(event) => {
          if (event.target === event.currentTarget) {
            event.currentTarget.querySelector<HTMLElement>("[role=treeitem]")?.focus();
          }
        }}
        onKeyDown={onTreeKeyDown}
        className="py-1 outline-none"
      >
        <Folder_ space={space} path="" depth={0} ignored={false} />
      </div>
    </TreeContext.Provider>
  );
}

/** One folder's children, fetched when shown and again whenever files change on disk. */
/** `ignored`: the folder itself is gitignored, so everything in it is too. */
function Folder_(props: { space: SpaceId; path: string; depth: number; ignored: boolean }) {
  const { space, path, depth } = props;
  const { data: entries, error } = useOnDisk(() => api.filesList(space, path), `${space}:${path}`);

  if (error) return <Row depth={depth} muted label={error} />;
  if (!entries) return null;
  if (entries.length === 0 && depth > 0)
    return <Row depth={depth} muted label={t("emptyFolder")} />;
  return (
    <>
      {entries.map((entry) => {
        const child = path ? `${path}/${entry.name}` : entry.name;
        const shown = props.ignored ? { ...entry, ignored: true } : entry;
        return entry.dir ? (
          <FolderRow key={child} space={space} path={child} entry={shown} depth={depth} />
        ) : (
          <FileRow key={child} space={space} path={child} entry={shown} depth={depth} />
        );
      })}
    </>
  );
}

/** Vertical 1px lines at each ancestor level, so deep nesting stays readable. */
function Guides({ depth }: { depth: number }) {
  return Array.from({ length: depth }, (_, level) => (
    <span
      // biome-ignore lint/suspicious/noArrayIndexKey: levels have no identity beyond position
      key={level}
      aria-hidden
      style={{ left: BASE + level * INDENT + 6 }}
      className="pointer-events-none absolute inset-y-0 w-px bg-line"
    />
  ));
}

const rowClass = (selected: boolean, color: string) =>
  `relative flex h-7 w-full items-center gap-2 pr-2 text-left text-body outline-none transition-colors focus-visible:ring-1 focus-visible:ring-primary/60 focus-visible:ring-inset ${
    selected ? "bg-selected text-fg" : `hover:bg-hover ${color}`
  }`;

function FolderRow(props: { space: SpaceId; path: string; entry: FileEntry; depth: number }) {
  const { selected, select, dirty } = useContext(TreeContext);
  const [open, setOpen] = useState(() => expandedIn(props.space).has(props.path));
  const toggle = () => {
    const set = expandedIn(props.space);
    if (open) {
      set.delete(props.path);
      // A hidden row can't hold the tab stop.
      if (selected?.startsWith(`${props.path}/`)) select(props.path);
    } else set.add(props.path);
    setOpen(!open);
  };
  const isSelected = selected === props.path;
  return (
    <>
      <button
        type="button"
        role="treeitem"
        aria-expanded={open}
        aria-selected={isSelected}
        aria-level={props.depth + 1}
        data-depth={props.depth}
        tabIndex={isSelected ? 0 : -1}
        onFocus={() => select(props.path)}
        onClick={toggle}
        style={{ paddingLeft: BASE + props.depth * INDENT }}
        className={rowClass(
          isSelected,
          props.entry.ignored ? "text-fg-subtle" : "text-fg-muted hover:text-fg",
        )}
      >
        <Guides depth={props.depth} />
        <ChevronRight
          aria-hidden
          className={`size-3.5 shrink-0 transition-transform duration-150 motion-reduce:transition-none ${open ? "rotate-90" : ""}`}
        />
        <FolderIcon open={open} />
        <span className="truncate">{props.entry.name}</span>
        {dirty.has(props.path) && (
          <span aria-hidden className="ml-auto size-1.5 shrink-0 rounded-full bg-warning" />
        )}
      </button>
      {open && (
        <Folder_
          space={props.space}
          path={props.path}
          depth={props.depth + 1}
          ignored={props.entry.ignored}
        />
      )}
    </>
  );
}

function FileRow(props: { space: SpaceId; path: string; entry: FileEntry; depth: number }) {
  const { selected, select, status } = useContext(TreeContext);
  const isSelected = selected === props.path;
  const letter = status.get(props.path);
  const color = letter
    ? (STATUS_COLOR[letter] ?? "text-warning")
    : props.entry.ignored
      ? "text-fg-subtle"
      : "text-fg-muted hover:text-fg";
  return (
    <button
      type="button"
      role="treeitem"
      aria-selected={isSelected}
      aria-level={props.depth + 1}
      data-depth={props.depth}
      tabIndex={isSelected ? 0 : -1}
      onFocus={() => select(props.path)}
      title={t("openFileHint")}
      onDoubleClick={() => void api.fileOpen(props.space, props.path)}
      onKeyDown={(event) => event.key === "Enter" && void api.fileOpen(props.space, props.path)}
      style={{ paddingLeft: BASE + props.depth * INDENT }}
      className={rowClass(isSelected, color)}
    >
      <Guides depth={props.depth} />
      {/* Same width as a folder's chevron, so names line up with sibling folders. */}
      <span aria-hidden className="size-3.5 shrink-0" />
      <FileIcon name={props.entry.name} />
      <span className={`truncate ${letter ? color : ""}`}>{props.entry.name}</span>
    </button>
  );
}

function Row({ depth, label, muted }: { depth: number; label: string; muted?: boolean }) {
  return (
    <p
      style={{ paddingLeft: BASE + depth * INDENT + 22 }}
      className={`relative h-7 truncate pr-2 text-body leading-7 ${muted ? "text-fg-subtle" : ""}`}
    >
      <Guides depth={depth} />
      {label}
    </p>
  );
}
