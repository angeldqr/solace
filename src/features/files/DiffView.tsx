import { ArrowLeft } from "lucide-react";
import { lazy, Suspense, useMemo, useState } from "react";
import { t } from "../../shared/i18n";
import { api, type Change, type SpaceId } from "../../shared/ipc";
import { Button } from "../../shared/ui/Button";
import { IconButton } from "../../shared/ui/IconButton";
import { updateSettings, useSettings } from "../settings/settings";
import { FileIcon } from "./fileIcons";
import { slash } from "./useChanges";
import { useOnDisk } from "./useOnDisk";

const PatchViewer = lazy(() => import("./PatchViewer"));

/** Longer patches ask before rendering: highlighting thousands of lines freezes the panel. */
const MAX_DIFF_LINES = 4000;

export function DiffView({
  space,
  change,
  onBack,
}: {
  space: SpaceId;
  change: Change;
  onBack: () => void;
}) {
  const ignoreWs = useSettings((s) => s.diffIgnoreWhitespace);
  const { data, error } = useOnDisk(
    () => api.gitDiff(space, change.path, change.from, change.status === "U", ignoreWs),
    `${space}:${change.status}:${change.path}:${ignoreWs}`,
  );
  const split = useSettings((s) => s.diffLayout) === "split";
  const [forced, setForced] = useState(false);
  const choose = (value: boolean) => updateSettings({ diffLayout: value ? "split" : "unified" });

  const path = slash(change.path);
  const slashAt = path.lastIndexOf("/");
  const name = path.slice(slashAt + 1);
  const dir = slashAt === -1 ? "" : path.slice(0, slashAt);
  const lines = useMemo(() => {
    let n = 1;
    for (let i = data?.indexOf("\n") ?? -1; i !== -1; i = data?.indexOf("\n", i + 1) ?? -1) n++;
    return n;
  }, [data]);
  const tooLong = data !== undefined && !forced && lines > MAX_DIFF_LINES;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 border-line border-b px-2">
        <IconButton label={t("back")} onClick={onBack}>
          <ArrowLeft aria-hidden />
        </IconButton>
        <FileIcon name={name} />
        <span className="flex min-w-0 flex-1 items-baseline gap-2 overflow-hidden text-body">
          <span className="min-w-0 truncate font-medium text-fg">{name}</span>
          {dir && <span className="min-w-0 shrink-[2] truncate text-fg-subtle text-xs">{dir}</span>}
        </span>
        {/* biome-ignore lint/a11y/useSemanticElements: a <fieldset> brings its own border and layout */}
        <div
          role="group"
          aria-label={t("diffStyle")}
          className="flex shrink-0 rounded-lg bg-hover p-0.5"
        >
          {[false, true].map((value) => (
            <button
              key={String(value)}
              type="button"
              aria-pressed={split === value}
              onClick={() => choose(value)}
              className={`h-6 rounded-md px-2 text-xs transition-colors ${
                split === value
                  ? "bg-pane text-fg shadow-sm ring-1 ring-line"
                  : "text-fg-muted hover:text-fg"
              }`}
            >
              {t(value ? "diffSplit" : "diffUnified")}
            </button>
          ))}
        </div>
      </div>
      {error !== undefined ? (
        <p className="px-3 py-3 text-body text-fg-subtle">{error}</p>
      ) : data === undefined ? null : !data.trim() ? (
        <p className="px-3 py-3 text-body text-fg-subtle">
          {t(ignoreWs ? "onlyWhitespaceChanges" : "noTextDiff")}
        </p>
      ) : tooLong ? (
        <div className="flex flex-col items-start gap-3 px-3 py-3 text-body text-fg-subtle">
          {t("diffTruncated")}
          <Button size="sm" onClick={() => setForced(true)}>
            {t("showAnyway")}
          </Button>
        </div>
      ) : (
        <Suspense
          fallback={<p className="px-3 py-3 text-body text-fg-subtle">{t("loadingDiff")}</p>}
        >
          <PatchViewer patch={data} split={split} />
        </Suspense>
      )}
    </div>
  );
}
