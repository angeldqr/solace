import { useState } from "react";
import { t } from "../../../shared/i18n";
import { Select } from "../../../shared/ui/Select";
import { Slider } from "../../../shared/ui/Slider";
import { Switch } from "../../../shared/ui/Switch";
import { Card, Row, Section } from "../layout";
import {
  type CursorStyle,
  resolveTheme,
  terminalFontFamily,
  updateSettings,
  useSettings,
  useSystemDark,
} from "../settings";

const FONTS = ["JetBrains Mono", "Cascadia Code", "Cascadia Mono", "Consolas"];
const OTHER = "other";
const LINE_HEIGHTS = ["1", "1.1", "1.2", "1.3", "1.4", "1.5"].map((v) => ({
  value: v,
  label: Number(v).toFixed(1),
}));

export function Terminal() {
  const s = useSettings();
  const [typing, setTyping] = useState(!FONTS.includes(s.fontFamily));
  const fonts = [
    ...FONTS.map((font) => ({ value: font, label: font })),
    { value: OTHER, label: t("otherFont") },
  ];
  const cursors: { value: CursorStyle; label: string }[] = [
    { value: "block", label: t("cursorBlock") },
    { value: "bar", label: t("cursorBar") },
    { value: "underline", label: t("cursorUnderline") },
  ];
  return (
    <Section title={t("terminalSection")}>
      <Card>
        <Row title={t("fontFamily")}>
          {typing && (
            <input
              value={s.fontFamily}
              onChange={(event) => updateSettings({ fontFamily: event.target.value })}
              aria-label={t("fontFamily")}
              className="h-8 w-40 rounded-lg border border-line bg-canvas/40 px-2.5 text-body text-fg"
            />
          )}
          <Select
            value={typing ? OTHER : s.fontFamily}
            options={fonts}
            label={t("fontFamily")}
            onChange={(value) => {
              setTyping(value === OTHER);
              if (value !== OTHER) updateSettings({ fontFamily: value });
            }}
          />
        </Row>
        <Row title={t("fontSize")}>
          <Slider
            value={s.fontSize}
            min={9}
            max={28}
            label={t("fontSize")}
            format={(v) => `${v}px`}
            onChange={(v) => updateSettings({ fontSize: v })}
          />
        </Row>
        <Row title={t("lineHeight")}>
          <Select
            value={String(s.lineHeight)}
            options={LINE_HEIGHTS}
            label={t("lineHeight")}
            onChange={(value) => updateSettings({ lineHeight: Number(value) })}
          />
        </Row>
        <Row title={t("cursorStyle")}>
          <Select
            value={s.cursorStyle}
            options={cursors}
            label={t("cursorStyle")}
            onChange={(value) => updateSettings({ cursorStyle: value })}
          />
        </Row>
        <Row title={t("cursorBlink")}>
          <Switch
            checked={s.cursorBlink}
            label={t("cursorBlink")}
            onCheckedChange={(value) => updateSettings({ cursorBlink: value })}
          />
        </Row>
      </Card>
      <Sample />
    </Section>
  );
}

/** The terminal font, size and colours as they will look. */
function Sample() {
  const s = useSettings();
  const systemDark = useSystemDark();
  const term = resolveTheme(s, systemDark).terminal;
  return (
    <div
      role="img"
      aria-label={t("terminalSample")}
      className="rounded-xl border border-line p-4"
      style={{
        background: term.background,
        color: term.foreground,
        fontFamily: terminalFontFamily(s.fontFamily),
        fontSize: s.fontSize,
        lineHeight: s.lineHeight,
      }}
    >
      <div>
        <span style={{ color: term.blue }}>~/solace $</span> pnpm test
      </div>
      <div style={{ color: term.green }}>✓ 8 passed</div>
      <div>
        <span style={{ color: term.blue }}>~/solace $</span>{" "}
        <span style={{ background: term.cursor }}>&nbsp;</span>
      </div>
    </div>
  );
}
