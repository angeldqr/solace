import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { t } from "../../shared/i18n";
import { api, type Change, type SpaceId } from "../../shared/ipc";
import { useOnDisk } from "./useOnDisk";

const STATUS_COLOR: Record<string, string> = {
  M: "text-warning",
  A: "text-success",
  U: "text-success",
  D: "text-danger",
  R: "text-info",
  "!": "text-danger",
  C: "text-info",
};

/** Long diffs are cut here; the full file is one double-click away in the editor. */
const MAX_DIFF_LINES = 4000;

export function ChangesView({ space }: { space: SpaceId }) {
  const { data: changes, error } = useOnDisk(() => api.gitStatus(space), String(space));
  const [selected, setSelected] = useState<Change | null>(null);

  if (selected) {
    return <DiffView space={space} change={selected} onBack={() => setSelected(null)} />;
  }
  if (error) return <Notice text={error} />;
  if (changes === undefined) return null;
  if (changes === null) return <Notice text={t("notARepository")} />;
  if (changes.length === 0) return <Notice text={t("noChanges")} />;

  const added = changes.reduce((n, c) => n + (c.additions ?? 0), 0);
  const removed = changes.reduce((n, c) => n + (c.deletions ?? 0), 0);
  return (
    <div className="flex flex-col">
      <p className="flex h-8 items-center gap-2 px-3 text-2xs text-fg-subtle">
        {t("changeCount", { n: changes.length })}
        <span className="ml-auto text-success tabular-nums">+{added}</span>
        <span className="text-danger tabular-nums">−{removed}</span>
      </p>
      {changes.map((change) => (
        <button
          type="button"
          key={change.path}
          title={change.path}
          onClick={() => setSelected(change)}
          className="flex h-7 w-full items-center gap-2 px-3 text-left text-fg-muted hover:bg-hover hover:text-fg"
        >
          <span
            className={`w-3 shrink-0 font-mono text-2xs ${STATUS_COLOR[change.status] ?? "text-fg-subtle"}`}
          >
            {change.status}
          </span>
          <span className="truncate" dir="rtl">
            {change.path}
          </span>
          {change.additions !== null && (
            <span className="ml-auto shrink-0 text-2xs tabular-nums">
              <span className="text-success">+{change.additions}</span>{" "}
              <span className="text-danger">−{change.deletions ?? 0}</span>
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

function DiffView({
  space,
  change,
  onBack,
}: {
  space: SpaceId;
  change: Change;
  onBack: () => void;
}) {
  const { data, error } = useOnDisk(
    () => api.gitDiff(space, change.path, change.from, change.status === "U"),
    `${space}:${change.status}:${change.path}`,
  );
  const diff = data ?? error ?? null;

  const lines = (diff ?? "").split("\n");
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-8 shrink-0 items-center gap-2 border-line border-b px-2">
        <button
          type="button"
          onClick={onBack}
          aria-label={t("back")}
          title={t("back")}
          className="grid size-6 place-items-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
        >
          <ArrowLeft aria-hidden className="size-3.5" />
        </button>
        <span className="truncate text-fg" title={change.path}>
          {change.path}
        </span>
      </div>
      <pre className="min-h-0 flex-1 overflow-auto py-2 font-mono text-2xs leading-relaxed select-text">
        {/* One box as wide as the longest line, so every row's colour spans the full scroll width. */}
        <div className="w-max min-w-full">
          {diff !== null && !diff.trim() && (
            <div className="px-3 text-fg-subtle">{t("noTextDiff")}</div>
          )}
          {lines.slice(0, MAX_DIFF_LINES).map((line, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: diff lines have no identity beyond position
            <div key={i} className={`px-3 ${lineColor(line)}`}>
              {line || " "}
            </div>
          ))}
          {lines.length > MAX_DIFF_LINES && (
            <div className="px-3 pt-2 text-fg-subtle">{t("diffTruncated")}</div>
          )}
        </div>
      </pre>
    </div>
  );
}

function lineColor(line: string) {
  if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("diff ")) {
    return "text-fg-subtle";
  }
  if (line.startsWith("+")) return "bg-success/10 text-success";
  if (line.startsWith("-")) return "bg-danger/10 text-danger";
  if (line.startsWith("@@")) return "text-info";
  return "text-fg-muted";
}

function Notice({ text }: { text: string }) {
  return <p className="px-3 py-3 text-fg-subtle">{text}</p>;
}
