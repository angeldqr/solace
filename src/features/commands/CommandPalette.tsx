import { Search } from "lucide-react";
import { type KeyboardEvent, useState } from "react";
import { t } from "../../shared/i18n";
import { current } from "../../shared/session";
import { focusTerminal } from "../terminals/registry";
import { commands, runCommand, usePalette } from "./commands";

export function CommandPalette() {
  const open = usePalette((s) => s.open);
  return open ? <PaletteDialog /> : null;
}

function PaletteDialog() {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const items = commands.filter((c) => t(c.title).toLowerCase().includes(query.toLowerCase()));

  const close = () => {
    usePalette.setState({ open: false });
    focusTerminal(current().pane);
  };
  const runAt = (i: number) => {
    const command = items[i];
    if (!command) return;
    close();
    runCommand(command);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      setIndex((i) => (i + step + items.length) % Math.max(items.length, 1));
    } else if (event.key === "Enter") {
      runAt(index);
    } else if (event.key === "Escape") {
      close();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex justify-center bg-black/50 pt-[14vh] motion-safe:animate-fade-in"
      onPointerDown={(event) => event.target === event.currentTarget && close()}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("commands")}
        className="h-fit w-[min(520px,90vw)] overflow-hidden rounded-xl border border-line bg-surface shadow-2xl shadow-black/70 motion-safe:animate-palette-in"
      >
        <div className="flex items-center gap-2.5 border-line border-b px-3.5">
          <Search className="size-4 text-fg-subtle" aria-hidden />
          <input
            // biome-ignore lint/a11y/noAutofocus: the palette exists to be typed into
            autoFocus
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setIndex(0);
            }}
            onKeyDown={onKeyDown}
            placeholder={t("searchCommands")}
            aria-label={t("searchCommands")}
            className="h-11 flex-1 bg-transparent text-[13px] text-fg outline-none placeholder:text-fg-subtle"
          />
        </div>
        <div role="listbox" aria-label={t("commands")} className="max-h-80 overflow-y-auto p-1.5">
          {items.map((command, i) => (
            <button
              type="button"
              role="option"
              aria-selected={i === index}
              key={command.id}
              tabIndex={-1}
              onPointerMove={() => setIndex(i)}
              onClick={() => runAt(i)}
              className="flex h-8 w-full cursor-default items-center justify-between rounded-md px-2.5 text-left text-fg-muted aria-selected:bg-hover aria-selected:text-fg"
            >
              <span>{t(command.title)}</span>
              {command.keys && (
                <kbd className="font-sans text-2xs text-fg-subtle">{command.keys}</kbd>
              )}
            </button>
          ))}
          {items.length === 0 && <p className="px-2.5 py-2 text-fg-subtle">{t("noMatches")}</p>}
        </div>
      </div>
    </div>
  );
}
