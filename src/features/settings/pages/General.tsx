import { useEffect, useState } from "react";
import { type MessageKey, t } from "../../../shared/i18n";
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
      <Presence />
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

function Presence() {
  const enabled = useSettings((s) => s.discordActivity);
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    void api
      .presenceAvailable()
      .then(setAvailable)
      .catch(() => {});
  }, []);
  const [status, setStatus] = useState(0);
  useEffect(() => {
    if (!enabled) {
      setStatus(0);
      return;
    }
    const poll = () => void api.presenceStatus().then(setStatus, () => {});
    poll();
    const id = setInterval(poll, 2000);
    return () => clearInterval(id);
  }, [enabled]);
  if (!available) return null;
  return (
    <Section title="Discord">
      <Card>
        <Row title={t("discordPresence")} description={t("discordPresenceHint")}>
          <Switch
            checked={enabled}
            label={t("discordPresence")}
            onCheckedChange={(value) => updateSettings({ discordActivity: value })}
          />
        </Row>
        {enabled && status > 0 && (
          <p role="status" className="px-4 pb-3 text-fg-muted text-xs">
            {t(`presenceStatus${status}` as MessageKey)}
          </p>
        )}
      </Card>
    </Section>
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
