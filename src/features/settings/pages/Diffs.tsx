import { t } from "../../../shared/i18n";
import { Select } from "../../../shared/ui/Select";
import { Switch } from "../../../shared/ui/Switch";
import { Card, Row, Section } from "../layout";
import { updateSettings, useSettings } from "../settings";

const PALETTES = {
  "red-green": ["#4ade80", "#f87171"],
  "blue-orange": ["#3b82f6", "#f97316"],
};

export function Diffs() {
  const s = useSettings();
  const [added, removed] = PALETTES[s.diffColors];
  return (
    <Section title={t("diffsPage")}>
      <Card>
        <Row title={t("diffStyle")}>
          <Select
            value={s.diffLayout}
            options={[
              { value: "unified", label: t("diffUnified") },
              { value: "split", label: t("diffSplit") },
            ]}
            label={t("diffStyle")}
            onChange={(diffLayout) => updateSettings({ diffLayout })}
          />
        </Row>
        <Row title={t("diffColours")}>
          <span
            role="img"
            aria-label={t("diffSwatch")}
            className="flex w-10 flex-col overflow-hidden rounded-md border border-line"
          >
            <span className="h-2.5" style={{ background: added }} />
            <span className="h-2.5" style={{ background: removed }} />
          </span>
          <Select
            value={s.diffColors}
            options={[
              { value: "red-green", label: t("diffRedGreen") },
              { value: "blue-orange", label: t("diffBlueOrange") },
            ]}
            label={t("diffColours")}
            onChange={(diffColors) => updateSettings({ diffColors })}
          />
        </Row>
        <Row title={t("wrapLines")}>
          <Switch
            checked={s.wordWrap}
            label={t("wrapLines")}
            onCheckedChange={(wordWrap) => updateSettings({ wordWrap })}
          />
        </Row>
        <Row title={t("ignoreWhitespace")} description={t("ignoreWhitespaceHint")}>
          <Switch
            checked={s.diffIgnoreWhitespace}
            label={t("ignoreWhitespace")}
            onCheckedChange={(diffIgnoreWhitespace) => updateSettings({ diffIgnoreWhitespace })}
          />
        </Row>
      </Card>
    </Section>
  );
}
