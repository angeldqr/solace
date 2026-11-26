import { Search } from "lucide-react";
import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { type MessageKey, t } from "../../shared/i18n";
import { current } from "../../shared/session";
import { Kbd } from "../../shared/ui/Kbd";
import { focusTerminal } from "../terminals/registry";
import { type Command, commands, matchCommand, runCommand, usePalette } from "./commands";

const GROUP_ORDER: MessageKey[] = ["groupTerminals", "groupLayout", "groupPanels", "groupApp"];

export function CommandPalette() {
  const open = usePalette((s) => s.open);
  return open ? <PaletteDialog /> : null;
}

function PaletteDialog() {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const listId = useId();
  const optionId = (command: Command) => `${listId}-${command.id}`;
  const grouped = query.trim() === "";
  // Grouped (empty query): commands follow the group order, which is also the keyboard order.
  const items = grouped
    ? GROUP_ORDER.flatMap((group) => commands.filter((c) => c.group === group))
    : commands.filter((c) => matchCommand(query, t(c.title), t(c.group)));
  const selected = items[index];

  const list = useRef<HTMLDivElement>(null);
  const fromKeyboard = useRef(false);
  // Only keyboard navigation scrolls: hovering near an edge must not make the list creep.
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs when the index changes
  useEffect(() => {
    if (!fromKeyboard.current) return;
    fromKeyboard.current = false;
    if (index === 0 && list.current) list.current.scrollTop = 0;
    else if (selected)
      document.getElementById(optionId(selected))?.scrollIntoView({ block: "nearest" });
  }, [index]);

  const option = (command: Command, i: number) => (
    <button
      key={command.id}
      type="button"
      role="option"
      id={optionId(command)}
      aria-selected={i === index}
      tabIndex={-1}
      onPointerMove={() => setIndex(i)}
      onClick={() => runAt(i)}
      className="group flex h-9 w-full cursor-default items-center gap-2.5 rounded-lg px-2.5 text-left text-body text-fg-muted aria-selected:bg-hover aria-selected:text-fg"
    >
      <command.icon
        className="size-4 shrink-0 text-fg-subtle group-aria-selected:text-fg"
        aria-hidden
      />
      <span className="truncate">{t(command.title)}</span>
      {command.keys && (
        <span className="ml-auto shrink-0">
          <Kbd keys={command.keys} />
        </span>
      )}
    </button>
  );

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
      fromKeyboard.current = true;
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
      className="fixed inset-0 z-50 flex justify-center backdrop-dim pt-[14vh] motion-safe:animate-fade-in"
      onPointerDown={(event) => event.target === event.currentTarget && close()}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t("commands")}
        className="glass h-fit w-[min(560px,92vw)] overflow-hidden rounded-2xl border border-line shadow-2xl shadow-black/50 motion-safe:animate-palette-in"
      >
        <div className="flex h-12 items-center gap-2.5 border-line border-b px-4">
          <Search className="size-4 shrink-0 text-fg-subtle" aria-hidden />
          <input
            // biome-ignore lint/a11y/noAutofocus: the palette exists to be typed into
            autoFocus
            role="combobox"
            aria-expanded
            aria-controls={listId}
            aria-activedescendant={selected ? optionId(selected) : undefined}
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setIndex(0);
            }}
            onKeyDown={onKeyDown}
            placeholder={t("searchCommands")}
            aria-label={t("searchCommands")}
            className="h-full flex-1 bg-transparent text-fg text-sm outline-none placeholder:text-fg-subtle"
          />
        </div>
        <div
          ref={list}
          id={listId}
          role="listbox"
          aria-label={t("commands")}
          className="max-h-96 overflow-y-auto p-1.5"
        >
          {grouped
            ? GROUP_ORDER.map((group) => {
                const headerId = `${listId}-${group}`;
                return (
                  // biome-ignore lint/a11y/useSemanticElements: a <fieldset> is not valid inside a listbox
                  <div key={group} role="group" aria-labelledby={headerId}>
                    <div
                      id={headerId}
                      className="px-2.5 pt-2.5 pb-1 font-medium text-fg-subtle text-xs"
                    >
                      {t(group)}
                    </div>
                    {items.map((c, i) => (c.group === group ? option(c, i) : null))}
                  </div>
                );
              })
            : items.map(option)}
          {items.length === 0 && <p className="px-2.5 py-2 text-fg-subtle">{t("noMatches")}</p>}
        </div>
      </div>
    </div>
  );
}
