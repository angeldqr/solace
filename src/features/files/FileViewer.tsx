import { ArrowLeft, Copy, ExternalLink, WrapText } from "lucide-react";
import {
  Component,
  lazy,
  type ReactNode,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { t } from "../../shared/i18n";
import { api, type SpaceId } from "../../shared/ipc";
import { Button } from "../../shared/ui/Button";
import { IconButton } from "../../shared/ui/IconButton";
import { updateSettings, useSettings } from "../settings/settings";
import { FileIcon } from "./fileIcons";
import { useOnDisk } from "./useOnDisk";

const CodeViewer = lazy(() => import("./CodeViewer"));
const MarkdownView = lazy(() => import("./MarkdownView"));

/** Longer files ask before rendering, like DiffView: highlighting thousands of lines freezes the panel. */
const MAX_LINES = 4000;
const MD_KEY = "solace.mdView";
const MARKDOWN = /\.(md|mdx|markdown)$/i;

/** A render failure in the viewer (a bad file, a parser bug) must not take the whole app down. */
class ViewerBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <p className="px-3 py-3 text-body text-fg-subtle">{t("fileViewError")}</p>
    ) : (
      this.props.children
    );
  }
}

const note = "px-3 py-3 text-body text-fg-subtle";

export function FileViewer({
  space,
  path,
  onBack,
}: {
  space: SpaceId;
  path: string;
  onBack: () => void;
}) {
  const { data, error } = useOnDisk(() => api.fileRead(space, path), `${space}:${path}`);
  const [rendered, setRendered] = useState(() => localStorage.getItem(MD_KEY) !== "source");
  const wrap = useSettings((s) => s.wordWrap);
  const [forced, setForced] = useState(false);
  const backButton = useRef<HTMLDivElement>(null);
  useEffect(() => backButton.current?.querySelector("button")?.focus(), []);

  const slashAt = path.lastIndexOf("/");
  const name = path.slice(slashAt + 1);
  const dir = slashAt === -1 ? "" : path.slice(0, slashAt);
  const markdown = MARKDOWN.test(name);
  const text = data?.kind === "text" ? data.text : undefined;
  const lines = useMemo(() => (text === undefined ? 0 : text.split("\n").length), [text]);
  const external = () => void api.fileOpen(space, path);
  const openButton = (
    <Button size="sm" onClick={external}>
      {t("openExternally")}
    </Button>
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 border-line border-b px-2">
        <div ref={backButton} className="grid">
          <IconButton label={t("back")} onClick={onBack}>
            <ArrowLeft aria-hidden />
          </IconButton>
        </div>
        <FileIcon name={name} />
        <span className="flex min-w-0 flex-1 items-baseline gap-2 overflow-hidden text-body">
          <span className="min-w-0 truncate font-medium text-fg">{name}</span>
          {dir && <span className="min-w-0 shrink-[2] truncate text-fg-subtle text-xs">{dir}</span>}
        </span>
        {markdown && (
          // biome-ignore lint/a11y/useSemanticElements: a <fieldset> brings its own border and layout
          <div
            role="group"
            aria-label={t("markdownView")}
            className="flex shrink-0 rounded-lg bg-hover p-0.5"
          >
            {[true, false].map((value) => (
              <button
                key={String(value)}
                type="button"
                aria-pressed={rendered === value}
                onClick={() => {
                  setRendered(value);
                  localStorage.setItem(MD_KEY, value ? "rendered" : "source");
                }}
                className={`h-6 rounded-md px-2 text-xs transition-colors ${
                  rendered === value
                    ? "bg-pane text-fg shadow-sm ring-1 ring-line"
                    : "text-fg-muted hover:text-fg"
                }`}
              >
                {t(value ? "mdRendered" : "mdSource")}
              </button>
            ))}
          </div>
        )}
        {!(markdown && rendered) && (
          <IconButton
            label={t("wrapLines")}
            pressed={wrap}
            onClick={() => updateSettings({ wordWrap: !wrap })}
          >
            <WrapText aria-hidden />
          </IconButton>
        )}
        <IconButton
          label={t("copyPath")}
          onClick={() => navigator.clipboard.writeText(path).catch(() => {})}
        >
          <Copy aria-hidden />
        </IconButton>
        <IconButton label={t("openExternally")} onClick={external}>
          <ExternalLink aria-hidden />
        </IconButton>
      </div>
      <ViewerBoundary key={path}>
        {error !== undefined ? (
          <p className={note}>{error}</p>
        ) : data === undefined ? null : data.kind === "binary" ? (
          <div className={`flex flex-col items-start gap-3 ${note}`}>
            {t("binaryFile")}
            {openButton}
          </div>
        ) : data.kind === "tooLarge" ? (
          <div className={`flex flex-col items-start gap-3 ${note}`}>
            {t("fileTooLarge")}
            {openButton}
          </div>
        ) : data.kind === "image" ? (
          <div className="grid min-h-0 flex-1 place-items-center overflow-auto bg-surface p-4">
            <img src={data.dataUrl} alt={name} className="max-w-full object-contain" />
          </div>
        ) : !forced && lines > MAX_LINES ? (
          <div className={`flex flex-col items-start gap-3 ${note}`}>
            {t("fileTruncated")}
            <Button size="sm" onClick={() => setForced(true)}>
              {t("showAnyway")}
            </Button>
          </div>
        ) : (
          <Suspense fallback={<p className={note}>{t("loadingFile")}</p>}>
            {markdown && rendered ? (
              <MarkdownView text={data.text} />
            ) : (
              <CodeViewer name={name} contents={data.text} />
            )}
          </Suspense>
        )}
      </ViewerBoundary>
    </div>
  );
}
