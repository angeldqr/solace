import { Dialog } from "@base-ui/react/dialog";
import {
  Bot,
  GitCompare,
  Info,
  Keyboard,
  Palette,
  RotateCcw,
  Search,
  SlidersHorizontal,
  SquareTerminal,
  Terminal as TerminalIcon,
  X,
} from "lucide-react";
import { type ComponentType, type RefObject, useMemo, useRef, useState } from "react";
import { t } from "../../shared/i18n";
import { current } from "../../shared/session";
import { Button } from "../../shared/ui/Button";
import { Kbd } from "../../shared/ui/Kbd";
import { focusTerminal } from "../terminals/registry";
import { useSettingsDialog } from "./dialog";
import { filterPages, PAGES, type PageId, RESETTABLE } from "./pages";
import { About } from "./pages/About";
import { Agents } from "./pages/Agents";
import { Appearance } from "./pages/Appearance";
import { Diffs } from "./pages/Diffs";
import { General } from "./pages/General";
import { Shell } from "./pages/Shell";
import { Shortcuts } from "./pages/Shortcuts";
import { Terminal } from "./pages/Terminal";
import { DEFAULTS, updateSettings } from "./settings";

const ICONS: Record<PageId, ComponentType<{ className?: string }>> = {
  general: SlidersHorizontal,
  appearance: Palette,
  terminal: SquareTerminal,
  diffs: GitCompare,
  agents: Bot,
  shell: TerminalIcon,
  shortcuts: Keyboard,
  about: Info,
};

const CONTENT: Record<PageId, ComponentType> = {
  general: General,
  appearance: Appearance,
  terminal: Terminal,
  diffs: Diffs,
  agents: Agents,
  shell: Shell,
  shortcuts: Shortcuts,
  about: About,
};

/** Reopening the dialog returns to the page it was left on. */
let lastPage: PageId = "general";

export function SettingsDialog() {
  const open = useSettingsDialog((s) => s.open);
  const search = useRef<HTMLInputElement>(null);
  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && close()}>
      <Dialog.Portal>
        <Dialog.Backdrop className="backdrop-dim fixed inset-0 z-50 transition-opacity duration-(--motion-duration) data-ending-style:opacity-0 data-starting-style:opacity-0" />
        <Dialog.Popup
          initialFocus={search}
          onKeyDown={(event) => {
            // Keys from portaled popups (an open Select) bubble through React too.
            if (!event.currentTarget.contains(event.target as Node)) return;
            const target = event.target as HTMLElement;
            if (event.key !== "/" || event.ctrlKey || event.metaKey || event.altKey) return;
            if (target.closest("input, textarea, select, [contenteditable]")) return;
            event.preventDefault();
            search.current?.focus();
          }}
          className="glass light:bg-surface fixed top-1/2 left-1/2 z-50 flex h-[min(720px,88vh)] w-[min(1040px,94vw)] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-2xl border border-line shadow-2xl shadow-black/50 outline-none transition-[opacity,scale] duration-(--motion-duration) ease-(--ease-drawer) data-ending-style:scale-[0.98] data-starting-style:scale-[0.98] data-ending-style:opacity-0 data-starting-style:opacity-0"
        >
          <SettingsBody search={search} />
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function close() {
  useSettingsDialog.setState({ open: false });
  focusTerminal(current().pane);
}

function SettingsBody({ search }: { search: RefObject<HTMLInputElement | null> }) {
  const [page, setPageState] = useState<PageId>(lastPage);
  const [query, setQuery] = useState("");
  // Bumped by "Restore defaults" so the page remounts and drops its local state.
  const [resets, setResets] = useState(0);
  const visible = useMemo(() => filterPages(query, t), [query]);

  const setPage = (id: PageId) => {
    lastPage = id;
    setPageState(id);
  };
  const resettable = RESETTABLE[page];
  const restore = () => {
    if (!resettable) return;
    updateSettings(Object.fromEntries(resettable.map((key) => [key, DEFAULTS[key]])));
    setResets(resets + 1);
  };
  const Content = CONTENT[page];
  const title = t(PAGES.find((p) => p.id === page)?.title ?? "settings");

  return (
    <>
      <nav className="flex w-60 shrink-0 flex-col gap-3 border-line border-r p-3">
        <div className="relative">
          <Search
            aria-hidden
            className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-fg-subtle"
          />
          <input
            ref={search}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && visible[0]) setPage(visible[0]);
            }}
            placeholder={t("searchSettings")}
            aria-label={t("searchSettings")}
            className="h-8 w-full rounded-lg border border-line bg-canvas/40 pr-8 pl-8 text-body text-fg placeholder:text-fg-subtle focus-visible:border-primary focus-visible:outline-offset-0"
          />
          {!query && (
            <span className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2">
              <Kbd keys="/" />
            </span>
          )}
        </div>
        <div className="flex flex-col gap-0.5">
          {visible.length === 0 && (
            <p className="px-2.5 py-1 text-fg-subtle text-xs">{t("noSettingsMatch")}</p>
          )}
          {PAGES.filter((p) => visible.includes(p.id)).map((p) => {
            const Icon = ICONS[p.id];
            const active = p.id === page;
            return (
              <button
                key={p.id}
                type="button"
                aria-current={active ? "page" : undefined}
                onClick={() => setPage(p.id)}
                className={`flex h-8 w-full items-center gap-2.5 rounded-lg px-2.5 text-body transition-colors duration-150 ${
                  active
                    ? "bg-selected font-medium text-fg"
                    : "text-fg-muted hover:bg-hover hover:text-fg"
                } ${p.id === "shortcuts" && !query ? "mt-4" : ""}`}
              >
                <Icon className="size-4" />
                {t(p.title)}
              </button>
            );
          })}
        </div>
      </nav>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-12 shrink-0 items-center gap-2 border-line border-b pr-3 pl-5">
          <Dialog.Title className="text-sm">
            <span className="text-fg-muted">{t("settings")}</span>
            <span aria-hidden className="px-1.5 text-fg-subtle">
              /
            </span>
            <span className="font-medium text-fg">{title}</span>
          </Dialog.Title>
          <div className="ml-auto flex items-center gap-2">
            {resettable && (
              <Button variant="ghost" size="sm" onClick={restore}>
                <RotateCcw aria-hidden />
                {t("restoreDefaults")}
              </Button>
            )}
            <Dialog.Close
              aria-label={t("close")}
              className="grid size-8 place-items-center rounded-lg text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg"
            >
              <X aria-hidden className="size-4" />
            </Dialog.Close>
          </div>
        </header>
        <div className="flex-1 overflow-y-auto">
          <div className="mx-auto flex max-w-[720px] flex-col gap-9 px-8 py-7">
            <Content key={`${page}-${resets}`} />
          </div>
        </div>
      </div>
    </>
  );
}
