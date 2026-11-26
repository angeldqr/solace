import { useEffect, useState } from "react";
import { t } from "../../../shared/i18n";
import { api } from "../../../shared/ipc";
import { Select } from "../../../shared/ui/Select";
import { Switch } from "../../../shared/ui/Switch";
import { Card, Hint, Row, Section } from "../layout";
import { flushSettings, type Language, updateSettings, useSettings } from "../settings";

export function General() {
  const language = useSettings((s) => s.language);
  const checkUpdates = useSettings((s) => s.checkUpdates);
  const languages: { value: Language; label: string }[] = [
    { value: "auto", label: t("languageAuto") },
    { value: "en", label: "English" },
    { value: "es", label: "Español" },
  ];
  return (
    <>
      <Section title={t("language")}>
        <Card>
          <Row title={t("interfaceLanguage")} description={t("languageHint")}>
            <Select
              value={language}
              options={languages}
              label={t("interfaceLanguage")}
              onChange={(value) => {
                updateSettings({ language: value });
                // Text is chosen at load; terminals and agents survive the reload.
                flushSettings();
                location.reload();
              }}
            />
          </Row>
        </Card>
      </Section>
      <Notifications />
      <Section title={t("updates")}>
        <Card>
          <Row title={t("checkUpdates")} description={t("privacyNote")}>
            <Switch
              checked={checkUpdates}
              label={t("checkUpdates")}
              onCheckedChange={(value) => updateSettings({ checkUpdates: value })}
            />
          </Row>
        </Card>
      </Section>
    </>
  );
}

function Notifications() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [systemOn, setSystemOn] = useState(true);
  useEffect(() => {
    void api.settingsGet().then((settings) => setEnabled(settings.notifications));
    void api.systemToastsEnabled().then(setSystemOn);
  }, []);
  if (enabled === null) return null;
  return (
    <Section title={t("notifications")}>
      <Card>
        <Row title={t("notifyAgents")}>
          <Switch
            checked={enabled}
            label={t("notifyAgents")}
            onCheckedChange={(value) => {
              setEnabled(value);
              void api.notificationsSet(value);
            }}
          />
        </Row>
      </Card>
      {enabled && !systemOn && <Hint>{t("systemToastsOff")}</Hint>}
    </Section>
  );
}
