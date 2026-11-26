import { useEffect, useState } from "react";
import { type MessageKey, t } from "../../../shared/i18n";
import { IS_MAC } from "../../../shared/platform";
import { Button } from "../../../shared/ui/Button";
import { Select } from "../../../shared/ui/Select";
import { Slider } from "../../../shared/ui/Slider";
import { Switch } from "../../../shared/ui/Switch";
import { Card, Hint, Row, Section } from "../layout";
import {
  fontStack,
  isHex,
  MAX_CSS,
  resolveTheme,
  setPreviewCss,
  updateSettings,
  useSettings,
  useSystemDark,
} from "../settings";
import { FONTS as CODE_FONTS } from "./Terminal";

const SWATCHES: [string, MessageKey][] = [
  ["#6366f1", "colourIndigo"],
  ["#8b5cf6", "colourViolet"],
  ["#3b82f6", "colourBlue"],
  ["#06b6d4", "colourCyan"],
  ["#10b981", "colourEmerald"],
  ["#f59e0b", "colourAmber"],
  ["#f43f5e", "colourRose"],
  ["#94a3b8", "colourSlate"],
];
const UI_FONTS = ["Inter", "Segoe UI", "System"];
const OTHER = "other";
const INPUT = "h-8 rounded-lg border border-line bg-canvas/40 px-2.5 text-body text-fg";

export function AccentSection() {
  const accent = useSettings((s) => s.accent);
  const [draft, setDraft] = useState(accent ?? "");
  const pick = (value: string | null) => {
    updateSettings({ accent: value });
    setDraft(value ?? "");
  };
  const invalid = draft !== "" && !isHex(draft);
  return (
    <Section title={t("accentColour")}>
      <div className="flex flex-wrap items-center gap-2.5">
        <Button
          size="sm"
          className="aria-pressed:border-primary"
          aria-pressed={accent === null}
          onClick={() => pick(null)}
        >
          {t("accentDefault")}
        </Button>
        {SWATCHES.map(([hex, name]) => (
          <button
            key={hex}
            type="button"
            aria-label={t(name)}
            aria-pressed={accent === hex}
            onClick={() => pick(hex)}
            style={{ background: hex }}
            className="size-6 rounded-full ring-fg ring-offset-2 ring-offset-canvas transition-shadow duration-(--motion-duration) hover:ring-1 aria-pressed:ring-2"
          />
        ))}
        <input
          type="color"
          aria-label={t("accentColour")}
          value={accent ?? "#6366f1"}
          onChange={(event) => pick(event.target.value)}
          className="size-7 cursor-pointer rounded-md border border-line bg-transparent p-0.5"
        />
        <input
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value);
            if (isHex(event.target.value)) updateSettings({ accent: event.target.value });
          }}
          placeholder="#6366f1"
          spellCheck={false}
          maxLength={7}
          aria-label={t("accentHex")}
          aria-invalid={invalid}
          className={`${INPUT} w-24 font-mono`}
        />
      </div>
      {invalid && <Hint tone="danger">{t("invalidHex")}</Hint>}
    </Section>
  );
}

/** A preset list plus "Other…", which reveals a free-text input (as the Terminal page does). */
function FontRow({
  title,
  value,
  presets,
  onChange,
}: {
  title: string;
  value: string;
  presets: string[];
  onChange: (value: string) => void;
}) {
  const [typing, setTyping] = useState(!presets.includes(value));
  const options = [
    ...presets.map((font) => ({ value: font, label: font === "System" ? t("fontSystem") : font })),
    { value: OTHER, label: t("otherFont") },
  ];
  return (
    <Row title={title}>
      {typing && (
        <input
          value={value}
          onChange={(event) => onChange(event.target.value)}
          aria-label={title}
          className={`${INPUT} w-40`}
        />
      )}
      <Select
        value={typing ? OTHER : value}
        options={options}
        label={title}
        onChange={(next) => {
          setTyping(next === OTHER);
          if (next !== OTHER) onChange(next);
        }}
      />
    </Row>
  );
}

export function TypographySection() {
  const s = useSettings();
  return (
    <Section title={t("typography")}>
      <Card>
        <FontRow
          title={t("uiFont")}
          value={s.uiFont}
          presets={UI_FONTS}
          onChange={(uiFont) => updateSettings({ uiFont })}
        />
        <FontRow
          title={t("codeFont")}
          value={s.codeFont}
          presets={CODE_FONTS}
          onChange={(codeFont) => updateSettings({ codeFont })}
        />
        <Row title={t("codeFontSize")}>
          <Slider
            value={s.codeFontSize}
            min={10}
            max={18}
            label={t("codeFontSize")}
            format={(v) => `${v}px`}
            onChange={(codeFontSize) => updateSettings({ codeFontSize })}
          />
        </Row>
        {/* Windows' text renderer ignores it. */}
        {IS_MAC && (
          <Row title={t("fontSmoothing")}>
            <Switch
              checked={s.fontSmoothing}
              label={t("fontSmoothing")}
              onCheckedChange={(fontSmoothing) => updateSettings({ fontSmoothing })}
            />
          </Row>
        )}
      </Card>
      <Preview />
    </Section>
  );
}

/** The two fonts as they will look: body text, and code in the theme's terminal colours. */
function Preview() {
  const s = useSettings();
  const term = resolveTheme(s, useSystemDark()).terminal;
  return (
    <div
      role="img"
      aria-label={t("typographyPreview")}
      className="flex flex-col gap-3 rounded-xl border border-line bg-surface/40 p-4"
    >
      <p className="text-body text-fg" style={{ fontFamily: fontStack(s.uiFont, "sans") }}>
        {t("typographySample")}
      </p>
      <pre
        className="rounded-lg p-3"
        style={{
          background: term.background,
          color: term.foreground,
          fontFamily: fontStack(s.codeFont, "mono"),
          fontSize: s.codeFontSize,
          lineHeight: `${Math.round(s.codeFontSize * 1.55)}px`,
        }}
      >
        <span style={{ color: term.brightBlack }}>{"// solace"}</span>
        {"\n"}
        <span style={{ color: term.magenta }}>const</span> agent ={" "}
        <span style={{ color: term.green }}>"claude"</span>;{"\n"}
        <span style={{ color: term.blue }}>run</span>(agent,{" "}
        <span style={{ color: term.yellow }}>3</span>);
      </pre>
    </div>
  );
}

const CONFIRM_SECONDS = 10;

export function CustomCssSection() {
  const applied = useSettings((s) => s.customCss);
  const [draft, setDraft] = useState(applied);
  // Seconds left to confirm a previewed stylesheet; null when nothing is being previewed.
  const [left, setLeft] = useState<number | null>(null);
  const revert = () => {
    setPreviewCss(null);
    setLeft(null);
  };
  const keep = () => {
    updateSettings({ customCss: draft });
    revert();
  };

  useEffect(() => {
    if (left === null) return;
    if (left <= 0) {
      setPreviewCss(null);
      setLeft(null);
      return;
    }
    const timer = setTimeout(() => setLeft(left - 1), 1000);
    return () => clearTimeout(timer);
  }, [left]);
  // Leaving the page (or closing settings) without confirming counts as reverting.
  useEffect(() => () => setPreviewCss(null), []);

  return (
    <Section title={t("customCss")}>
      <Hint>{t("customCssHint")}</Hint>
      {left !== null && (
        <div
          role="alert"
          className="flex items-center gap-3 rounded-lg border border-primary bg-surface/40 px-3 py-2 text-body text-fg"
        >
          <span className="flex-1">{t("keepStyles", { n: left })}</span>
          <Button size="sm" variant="primary" onClick={keep}>
            {t("keep")}
          </Button>
          <Button size="sm" onClick={revert}>
            {t("revert")}
          </Button>
        </div>
      )}
      <textarea
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        rows={8}
        maxLength={MAX_CSS}
        spellCheck={false}
        aria-label={t("customCss")}
        className="rounded-lg border border-line bg-canvas/40 p-2 font-mono text-fg text-xs"
      />
      <div className="flex gap-2">
        <Button
          variant="primary"
          disabled={draft === applied || left !== null}
          onClick={() => {
            setPreviewCss(draft);
            setLeft(CONFIRM_SECONDS);
          }}
        >
          {t("apply")}
        </Button>
        <Button
          disabled={!draft && !applied}
          onClick={() => {
            revert();
            setDraft("");
            updateSettings({ customCss: "" });
          }}
        >
          {t("clear")}
        </Button>
      </div>
    </Section>
  );
}
