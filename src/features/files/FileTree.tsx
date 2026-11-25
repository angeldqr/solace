import { ChevronRight, File, Folder, FolderOpen } from "lucide-react";
import { useState } from "react";
import { t } from "../../shared/i18n";
import { api, type FileEntry, type SpaceId } from "../../shared/ipc";
import { useOnDisk } from "./useOnDisk";

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

export function FileTree({ space }: { space: SpaceId }) {
  return (
    <div role="tree" aria-label={t("files")} className="py-1">
      <Folder_ space={space} path="" depth={0} ignored={false} />
    </div>
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

function FolderRow(props: { space: SpaceId; path: string; entry: FileEntry; depth: number }) {
  const [open, setOpen] = useState(() => expandedIn(props.space).has(props.path));
  const toggle = () => {
    const set = expandedIn(props.space);
    if (open) set.delete(props.path);
    else set.add(props.path);
    setOpen(!open);
  };
  return (
    <>
      <button
        type="button"
        role="treeitem"
        aria-expanded={open}
        onClick={toggle}
        style={{ paddingLeft: 8 + props.depth * 12 }}
        className={`flex h-6 w-full items-center gap-1.5 pr-2 text-left hover:bg-hover ${
          props.entry.ignored ? "text-fg-subtle" : "text-fg-muted hover:text-fg"
        }`}
      >
        <ChevronRight
          aria-hidden
          className={`size-3 shrink-0 transition-transform duration-150 motion-reduce:transition-none ${open ? "rotate-90" : ""}`}
        />
        {open ? (
          <FolderOpen aria-hidden className="size-3.5 shrink-0" />
        ) : (
          <Folder aria-hidden className="size-3.5 shrink-0" />
        )}
        <span className="truncate">{props.entry.name}</span>
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
  return (
    <button
      type="button"
      role="treeitem"
      title={t("openFileHint")}
      onDoubleClick={() => void api.fileOpen(props.space, props.path)}
      onKeyDown={(event) => event.key === "Enter" && void api.fileOpen(props.space, props.path)}
      style={{ paddingLeft: 8 + props.depth * 12 + 16 }}
      className={`flex h-6 w-full items-center gap-1.5 pr-2 text-left hover:bg-hover focus-visible:bg-hover ${
        props.entry.ignored ? "text-fg-subtle" : "text-fg-muted hover:text-fg"
      }`}
    >
      <File aria-hidden className="size-3.5 shrink-0" />
      <span className="truncate">{props.entry.name}</span>
    </button>
  );
}

function Row({ depth, label, muted }: { depth: number; label: string; muted?: boolean }) {
  return (
    <p
      style={{ paddingLeft: 8 + depth * 12 + 16 }}
      className={`h-6 truncate pr-2 leading-6 ${muted ? "text-fg-subtle" : ""}`}
    >
      {label}
    </p>
  );
}
