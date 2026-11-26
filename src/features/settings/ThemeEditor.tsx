import { useEffect, useState } from "react";
import { type MessageKey, t } from "../../shared/i18n";
import { Button } from "../../shared/ui/Button";
import { Hint } from "./layout";
import { updateSettings, useSettings } from "./settings";
import type { Theme, UiTokens } from "./themes";

const UI_TOKENS: [keyof UiTokens, MessageKey][] = [
  ["canvas", "tokenCanvas"],
  ["surface", "tokenSurface"],
  ["pane", "tokenPane"],
  ["fg", "tokenFg"],
  ["fg-muted", "tokenFgMuted"],
  ["fg-subtle", "tokenFgSubtle"],
  ["line", "tokenLine"],
  ["line-strong", "tokenLineStrong"],
  ["hover", "tokenHover"],
  ["selected", "tokenSelected"],
  ["primary", "tokenPrimary"],
  ["primary-soft", "tokenPrimarySoft"],
];
const TERMINAL_TOKENS = [
  ["background", "tokenTermBackground"],
  ["foreground", "tokenTermForeground"],
  ["cursor", "tokenTermCursor"],
] as const;
const HEX = /^#[0-9a-f]{6}$/i;
/** Any CSS colour except the ones that depend on the page or on variables a theme can't resolve. */
const validColour = (v: string) =>
  CSS.supports("color", v) &&
  !/^(inherit|initial|unset|revert|revert-layer|currentcolor)$/i.test(v.trim()) &&
  !/var\(/i.test(v);

/** One colour: swatch, native picker for plain hex values, and a text field for any CSS colour. */
function ColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const valid = validColour(draft);
  return (
    <div className="flex items-center gap-2 text-xs">
      <span className="w-32 shrink-0 truncate text-fg-muted">{label}</span>
      {HEX.test(value) ? (
        <input
          type="color"
          value={value}
          aria-label={t("pickerFor", { name: label })}
          onChange={(event) => onChange(event.target.value)}
          className="size-6 shrink-0 cursor-pointer rounded border border-line bg-transparent p-0"
        />
      ) : (
        <span
          aria-hidden
          className="size-6 shrink-0 rounded border border-line"
          style={{ background: value }}
        />
      )}
      <input
        value={draft}
        spellCheck={false}
        aria-label={label}
        aria-invalid={!valid}
        title={valid ? undefined : t("invalidColour")}
        onChange={(event) => {
          setDraft(event.target.value);
          if (validColour(event.target.value)) onChange(event.target.value);
        }}
        className={`h-7 min-w-0 flex-1 rounded-md border bg-canvas/40 px-2 font-mono text-fg ${
          valid ? "border-line" : "border-danger"
        }`}
      />
    </div>
  );
}

/** Edits the custom theme live; the caller makes sure one exists before opening it. */
export function ThemeEditor({ onDone, onImport }: { onDone: () => void; onImport: () => void }) {
  const custom = useSettings((s) => s.custom);
  const [copied, setCopied] = useState(false);
  if (!custom) return null;
  const set = (group: "ui" | "terminal", key: string, value: string) =>
    updateSettings({ custom: { ...custom, [group]: { ...custom[group], [key]: value } } as Theme });
  return (
    <div className="flex flex-col gap-3 rounded-xl border border-line bg-surface/40 p-4">
      <h4 className="font-medium text-body text-fg">{t("editThemeColours")}</h4>
      <div className="grid grid-cols-2 gap-x-6 gap-y-2">
        {UI_TOKENS.map(([key, label]) => (
          <ColorField
            key={key}
            label={t(label)}
            value={custom.ui[key]}
            onChange={(value) => set("ui", key, value)}
          />
        ))}
        {TERMINAL_TOKENS.map(([key, label]) => (
          <ColorField
            key={key}
            label={t(label)}
            value={custom.terminal[key] ?? "#000000"}
            onChange={(value) => set("terminal", key, value)}
          />
        ))}
      </div>
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          onClick={() =>
            navigator.clipboard.writeText(JSON.stringify(custom, null, 2)).then(
              () => setCopied(true),
              () => {},
            )
          }
        >
          {t("copyJson")}
        </Button>
        <Button size="sm" onClick={onImport}>
          {t("importVsCode")}
        </Button>
        {copied && <Hint>{t("jsonCopied")}</Hint>}
        <Button size="sm" variant="primary" className="ml-auto" onClick={onDone}>
          {t("done")}
        </Button>
      </div>
    </div>
  );
}
