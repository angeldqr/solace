import { Radio } from "@base-ui/react/radio";
import { RadioGroup } from "@base-ui/react/radio-group";
import { Pencil, Upload } from "lucide-react";
import { useState } from "react";
import { t } from "../../../shared/i18n";
import { Button } from "../../../shared/ui/Button";
import { Slider } from "../../../shared/ui/Slider";
import { Card, Hint, Row, Section } from "../layout";
import {
  activeTheme,
  completeTheme,
  type Appearance as Mode,
  updateSettings,
  useSettings,
} from "../settings";
import { ThemeEditor } from "../ThemeEditor";
import { ModePreview, ThemeOrb } from "../ThemePreview";
import { FAMILIES } from "../themes";
import { fromVsCode, parseJsonc } from "../vscodeTheme";
import { AccentSection, CustomCssSection, TypographySection } from "./Personalize";

const MODES: { value: Mode; label: () => string }[] = [
  { value: "system", label: () => t("appearanceSystem") },
  { value: "light", label: () => t("appearanceLight") },
  { value: "dark", label: () => t("appearanceDark") },
];

/** A Solace theme ({name, dark, ui, terminal}) or a VS Code colour theme. */
function parseTheme(source: string) {
  let json: unknown;
  try {
    json = parseJsonc(source);
  } catch {
    // fromVsCode reports unparseable input.
  }
  const solace = json && typeof json === "object" && "ui" in json && "terminal" in json;
  const imported = solace ? completeTheme(json) : null;
  return imported ?? fromVsCode(source);
}

const TILE =
  "rounded-xl border border-line text-left transition-colors duration-150 hover:border-line-strong data-checked:border-primary data-checked:ring-1 data-checked:ring-primary data-disabled:pointer-events-none data-disabled:opacity-50";

export function Appearance() {
  const { theme, appearance, uiScale, contrast, glassOpacity, motionMs } = useSettings();
  const isCustom = theme === "custom";
  return (
    <>
      <Section title={t("appearanceMode")}>
        <RadioGroup
          value={appearance}
          onValueChange={(value) => updateSettings({ appearance: value as Mode })}
          disabled={isCustom}
          aria-label={t("appearanceMode")}
          aria-describedby={isCustom ? "custom-theme-hint" : undefined}
          className="grid grid-cols-3 gap-3"
        >
          {MODES.map((mode) => (
            <Radio.Root
              key={mode.value}
              value={mode.value}
              className={`flex flex-col gap-2 p-2 ${TILE}`}
            >
              <ModePreview mode={mode.value} familyId={theme} />
              <span className="px-1 pb-0.5 font-medium text-xs">{mode.label()}</span>
            </Radio.Root>
          ))}
        </RadioGroup>
        {isCustom && <Hint id="custom-theme-hint">{t("customThemeHint")}</Hint>}
      </Section>
      <ThemeSection />
      <AccentSection />
      <TypographySection />
      <Section title={t("interfaceSection")}>
        <Card>
          <Row title={t("interfaceSize")} description={t("interfaceSizeHint")}>
            <Slider
              value={uiScale}
              min={14}
              max={18}
              label={t("interfaceSize")}
              format={(v) => `${v}px`}
              onChange={(v) => updateSettings({ uiScale: v })}
            />
          </Row>
          <Row title={t("contrast")} description={t("contrastHint")}>
            <Slider
              value={contrast}
              min={50}
              max={200}
              step={5}
              label={t("contrast")}
              format={(v) => `${v}%`}
              onChange={(v) => updateSettings({ contrast: v })}
            />
          </Row>
          <Row title={t("glass")} description={t("glassHint")}>
            <Slider
              value={glassOpacity}
              min={40}
              max={100}
              step={5}
              label={t("glass")}
              format={(v) => `${v}%`}
              onChange={(v) => updateSettings({ glassOpacity: v })}
            />
          </Row>
          <Row title={t("animationSpeed")}>
            <Slider
              value={motionMs}
              min={0}
              max={400}
              step={25}
              label={t("animationSpeed")}
              format={(v) => (v === 0 ? t("motionOff") : `${v} ms`)}
              onChange={(v) => updateSettings({ motionMs: v })}
            />
          </Row>
        </Card>
      </Section>
      <CustomCssSection />
    </>
  );
}

function ThemeSection() {
  const { theme, custom } = useSettings();
  const [importing, setImporting] = useState(false);
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [source, setSource] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const importTheme = () => {
    try {
      const imported = parseTheme(source);
      updateSettings({ custom: imported, theme: "custom" });
      setMessage({ ok: true, text: t("themeImported", { name: imported.name }) });
      setSource("");
    } catch (error) {
      setMessage({ ok: false, text: (error as Error).message });
    }
  };

  // A copy of the theme on screen becomes the custom theme.
  const copyCurrent = () => {
    const base = activeTheme();
    updateSettings({
      theme: "custom",
      custom: { ...base, id: "custom", name: t("customSuffix", { name: base.name }) },
    });
    setEditing(true);
  };
  // An existing custom theme is never overwritten silently: Customize opens it instead.
  const customize = () => {
    if (theme === "custom" && custom) setEditing(!editing);
    else if (custom) {
      updateSettings({ theme: "custom" });
      setEditing(true);
    } else copyCurrent();
  };

  return (
    <Section
      title={t("theme")}
      action={
        <div className="flex gap-2">
          {custom && theme !== "custom" && (
            <Button
              size="sm"
              onBlur={() => setConfirming(false)}
              onClick={() => {
                if (!confirming) return setConfirming(true);
                setConfirming(false);
                copyCurrent();
              }}
            >
              {confirming ? t("replaceCustomConfirm") : t("startFromCurrent")}
            </Button>
          )}
          <Button size="sm" aria-expanded={editing} onClick={customize}>
            <Pencil aria-hidden />
            {t("customize")}
          </Button>
          <Button size="sm" aria-expanded={importing} onClick={() => setImporting(!importing)}>
            <Upload aria-hidden />
            {t("importVsCode")}
          </Button>
        </div>
      }
    >
      <RadioGroup
        value={theme}
        onValueChange={(value) => updateSettings({ theme: value as string })}
        aria-label={t("theme")}
        className="grid grid-cols-3 gap-3"
      >
        {FAMILIES.map((family) => (
          <Radio.Root
            key={family.id}
            value={family.id}
            className={`flex flex-col items-center gap-3 p-4 ${TILE}`}
          >
            <span className="flex items-center gap-2">
              {family.light && <ThemeOrb theme={family.light} />}
              <ThemeOrb theme={family.dark} />
            </span>
            <span className="font-medium text-sm">{family.name}</span>
          </Radio.Root>
        ))}
        {custom && (
          <Radio.Root value="custom" className={`flex flex-col items-center gap-3 p-4 ${TILE}`}>
            <ThemeOrb theme={custom} />
            <span className="max-w-full truncate font-medium text-sm">{custom.name}</span>
          </Radio.Root>
        )}
      </RadioGroup>
      {editing && theme === "custom" && (
        <ThemeEditor onDone={() => setEditing(false)} onImport={() => setImporting(true)} />
      )}
      {importing && (
        <div className="flex flex-col gap-2">
          <Hint>{t("importVsCodeHint")}</Hint>
          <textarea
            value={source}
            onChange={(event) => setSource(event.target.value)}
            rows={5}
            spellCheck={false}
            aria-label={t("importVsCode")}
            className="rounded-lg border border-line bg-canvas/40 p-2 font-mono text-xs text-fg"
          />
          <div className="flex items-center gap-3">
            <Button variant="primary" onClick={importTheme} disabled={!source.trim()}>
              {t("import")}
            </Button>
            {message && (
              <p
                role="status"
                className={`text-xs ${message.ok ? "text-fg-muted" : "text-danger"}`}
              >
                {message.text}
              </p>
            )}
          </div>
        </div>
      )}
    </Section>
  );
}
