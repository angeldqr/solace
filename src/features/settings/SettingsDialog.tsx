import { getVersion } from "@tauri-apps/api/app";
import { X } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { type MessageKey, t } from "../../shared/i18n";
import { api, type Shell } from "../../shared/ipc";
import { current } from "../../shared/session";
import { focusTerminal } from "../terminals/registry";
import { useUpdate } from "../updates/updates";
import { useSettingsDialog } from "./dialog";
import { type CursorStyle, type Language, updateSettings, useSettings } from "./settings";
import { THEMES, type Theme } from "./themes";
import { fromVsCode } from "./vscodeTheme";

const FONTS = ["JetBrains Mono", "Cascadia Code", "Cascadia Mono", "Consolas"];

export function SettingsDialog() {
  const open = useSettingsDialog((s) => s.open);
  return open ? <Dialog /> : null;
}

function close() {
  useSettingsDialog.setState({ open: false });
  focusTerminal(current().pane);
}

function Dialog() {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Opened with Ctrl+, the keyboard is still in the terminal: take it, or typing (and Escape)
    // would go to the shell behind the dialog.
    box.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
  return (
    <div
      className="fixed inset-0 z-50 flex justify-center bg-black/50 pt-[8vh] motion-safe:animate-fade-in"
      onPointerDown={(event) => event.target === event.currentTarget && close()}
    >
      <div
        ref={box}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        className="flex h-fit max-h-[84vh] w-[min(620px,92vw)] flex-col overflow-hidden rounded-xl border border-line bg-surface shadow-2xl shadow-black/50 motion-safe:animate-palette-in"
      >
        <header className="flex h-12 shrink-0 items-center border-line border-b pr-2 pl-5">
          <h2 id="settings-title" className="font-medium text-[14px]">
            {t("settings")}
          </h2>
          <button
            type="button"
            onClick={close}
            aria-label={t("close")}
            className="ml-auto grid size-8 place-items-center rounded-md text-fg-muted hover:bg-hover hover:text-fg"
          >
            <X aria-hidden className="size-4" />
          </button>
        </header>
        <div className="flex flex-col gap-7 overflow-y-auto px-5 py-5">
          <Appearance />
          <TerminalSection />
          <ShellSection />
          <LanguageSection />
          <About />
        </div>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: MessageKey; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-medium text-fg text-xs">{t(title)}</h3>
      {children}
    </section>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    // biome-ignore lint/a11y/noLabelWithoutControl: the control is passed in as a child
    <label className="flex items-center justify-between gap-4 text-fg-muted">
      <span>{label}</span>
      {children}
    </label>
  );
}

const control =
  "h-8 rounded-md border border-line bg-canvas px-2 text-[13px] text-fg outline-none focus-visible:border-primary";

function Appearance() {
  const { theme, custom } = useSettings();
  const [source, setSource] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const themes = custom ? [...THEMES, custom] : THEMES;

  const importTheme = () => {
    try {
      const imported = fromVsCode(source);
      updateSettings({ custom: imported, theme: "custom" });
      setMessage({ ok: true, text: t("themeImported", { name: imported.name }) });
      setSource("");
    } catch (error) {
      setMessage({ ok: false, text: (error as Error).message });
    }
  };

  return (
    <Section title="appearance">
      <div role="radiogroup" aria-label={t("theme")} className="grid grid-cols-3 gap-2">
        {themes.map((option) => (
          <ThemeCard key={option.id} theme={option} selected={option.id === theme} />
        ))}
      </div>
      <details className="group text-fg-muted">
        <summary className="cursor-pointer select-none hover:text-fg">{t("importVsCode")}</summary>
        <div className="mt-2 flex flex-col gap-2">
          <p className="text-2xs text-fg-subtle">{t("importVsCodeHint")}</p>
          <textarea
            value={source}
            onChange={(event) => setSource(event.target.value)}
            rows={5}
            spellCheck={false}
            aria-label={t("importVsCode")}
            className={`${control} h-auto py-2 font-mono text-2xs`}
          />
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={importTheme}
              disabled={!source.trim()}
              className="h-8 rounded-md bg-primary px-3 font-medium text-on-primary transition-[filter] hover:brightness-110 disabled:opacity-40"
            >
              {t("import")}
            </button>
            {message && (
              <p role="status" className={message.ok ? "text-fg-muted" : "text-red-500"}>
                {message.text}
              </p>
            )}
          </div>
        </div>
      </details>
    </Section>
  );
}

/** A miniature of the theme: canvas, a pane with a few coloured terminal lines, the accent. */
function ThemeCard({ theme, selected }: { theme: Theme; selected: boolean }) {
  const term = theme.terminal;
  return (
    // biome-ignore lint/a11y/useSemanticElements: a native radio can't hold the theme preview
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={() => updateSettings({ theme: theme.id })}
      className={`flex flex-col gap-2 rounded-lg border p-2 text-left transition-colors duration-150 ${
        selected ? "border-primary" : "border-line hover:border-line-strong"
      }`}
    >
      <span
        aria-hidden
        className="flex h-14 flex-col justify-center gap-1 rounded-md px-2"
        style={{ background: theme.ui.canvas }}
      >
        <span
          className="flex flex-col gap-1 rounded px-1.5 py-1"
          style={{ background: term.background }}
        >
          <span className="h-1 w-3/4 rounded-full" style={{ background: term.foreground }} />
          <span className="flex gap-1">
            {[term.green, term.blue, term.yellow, term.red].map((color) => (
              <span key={color} className="h-1 w-3 rounded-full" style={{ background: color }} />
            ))}
          </span>
        </span>
        <span className="h-1 w-6 rounded-full" style={{ background: theme.ui.primary }} />
      </span>
      <span className="truncate px-0.5 text-fg">{theme.name}</span>
    </button>
  );
}

function TerminalSection() {
  const s = useSettings();
  const [typing, setTyping] = useState(!FONTS.includes(s.fontFamily));
  return (
    <Section title="terminalSection">
      <Field label={t("fontFamily")}>
        <span className="flex gap-2">
          <select
            value={typing ? "other" : s.fontFamily}
            onChange={(event) => {
              const other = event.target.value === "other";
              setTyping(other);
              if (!other) updateSettings({ fontFamily: event.target.value });
            }}
            className={control}
          >
            {FONTS.map((font) => (
              <option key={font} value={font}>
                {font}
              </option>
            ))}
            <option value="other">{t("otherFont")}</option>
          </select>
          {typing && (
            <input
              value={s.fontFamily}
              onChange={(event) => updateSettings({ fontFamily: event.target.value })}
              aria-label={t("fontFamily")}
              className={`${control} w-36`}
            />
          )}
        </span>
      </Field>
      <Field label={t("fontSize")}>
        <input
          type="number"
          min={9}
          max={28}
          value={s.fontSize}
          onChange={(event) => {
            const size = Number(event.target.value);
            if (size >= 9 && size <= 28) updateSettings({ fontSize: size });
          }}
          className={`${control} w-20`}
        />
      </Field>
      <Field label={t("lineHeight")}>
        <select
          value={s.lineHeight}
          onChange={(event) => updateSettings({ lineHeight: Number(event.target.value) })}
          className={control}
        >
          {[1, 1.1, 1.2, 1.3, 1.4, 1.5].map((value) => (
            <option key={value} value={value}>
              {value.toFixed(1)}
            </option>
          ))}
        </select>
      </Field>
      <Field label={t("cursorStyle")}>
        <select
          value={s.cursorStyle}
          onChange={(event) => updateSettings({ cursorStyle: event.target.value as CursorStyle })}
          className={control}
        >
          <option value="block">{t("cursorBlock")}</option>
          <option value="bar">{t("cursorBar")}</option>
          <option value="underline">{t("cursorUnderline")}</option>
        </select>
      </Field>
      <Field label={t("cursorBlink")}>
        <input
          type="checkbox"
          checked={s.cursorBlink}
          onChange={(event) => updateSettings({ cursorBlink: event.target.checked })}
          className="size-4 accent-[var(--color-primary)]"
        />
      </Field>
    </Section>
  );
}

function ShellSection() {
  const [shells, setShells] = useState<Shell[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api.shellsList().then(setShells);
    void api.settingsGet().then((settings) => setSelected(settings.shell ?? ""));
  }, []);

  const choose = (path: string) => {
    setSelected(path);
    api.shellSet(path || null).then(
      () => setError(null),
      (e) => setError(String(e)),
    );
  };

  return (
    <Section title="shell">
      <Field label={t("shellForNewTerminals")}>
        <select
          value={selected}
          onChange={(event) => choose(event.target.value)}
          className={control}
        >
          <option value="">{t("shellDefault")}</option>
          {shells.map((shell) => (
            <option key={shell.path} value={shell.path}>
              {shell.name}
            </option>
          ))}
        </select>
      </Field>
      {error && <p className="text-red-500">{error}</p>}
    </Section>
  );
}

function LanguageSection() {
  const language = useSettings((s) => s.language);
  return (
    <Section title="language">
      <Field label={t("interfaceLanguage")}>
        <select
          value={language}
          onChange={(event) => {
            updateSettings({ language: event.target.value as Language });
            // Text is chosen at load; terminals and agents survive the reload.
            location.reload();
          }}
          className={control}
        >
          <option value="auto">{t("languageAuto")}</option>
          <option value="en">English</option>
          <option value="es">Español</option>
        </select>
      </Field>
    </Section>
  );
}

function About() {
  const [version, setVersion] = useState("");
  const update = useUpdate((s) => s.ready);
  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);
  return (
    <Section title="about">
      <p className="text-fg-muted">
        Solace {version}
        {update && (
          <span className="text-primary-soft">
            {" "}
            · {t("updateReady", { version: update.version })}
          </span>
        )}
      </p>
    </Section>
  );
}
