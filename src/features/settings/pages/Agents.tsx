import { useEffect, useState } from "react";
import { t } from "../../../shared/i18n";
import { api } from "../../../shared/ipc";
import { Switch } from "../../../shared/ui/Switch";
import { Card, Hint, Row, Section } from "../layout";

export function Agents() {
  const [installed, setInstalled] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.claudeIntegrationInstalled().then(setInstalled, (e) => setError(String(e)));
  }, []);
  const toggle = (enabled: boolean) => {
    setError(null);
    setBusy(true);
    api
      .claudeIntegrationSet(enabled)
      .then(
        () => setInstalled(enabled),
        (e) => setError(String(e)),
      )
      .finally(() => setBusy(false));
  };
  return (
    <Section title={t("agents")}>
      <Card>
        <Row title={t("claudeIntegration")} description={t("claudeIntegrationHint")}>
          <Switch
            checked={installed ?? false}
            disabled={installed === null || busy}
            label={t("claudeIntegration")}
            onCheckedChange={toggle}
          />
        </Row>
      </Card>
      {error && <Hint tone="danger">{error}</Hint>}
    </Section>
  );
}
