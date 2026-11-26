import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { t } from "../../../shared/i18n";
import { Button } from "../../../shared/ui/Button";
import { restartToUpdate, useUpdate } from "../../updates/updates";
import { Card, Row, Section } from "../layout";
import { useSettings } from "../settings";

export function About() {
  const [version, setVersion] = useState("");
  const update = useUpdate((s) => s.ready);
  const checkUpdates = useSettings((s) => s.checkUpdates);
  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);
  return (
    <Section title="Solace">
      <Card>
        <Row title={t("version")}>
          <span className="text-fg-muted text-xs">Solace {version}</span>
          {update ? (
            <Button variant="primary" size="sm" onClick={() => void restartToUpdate()}>
              {t("restartToUpdate", { version: update.version })}
            </Button>
          ) : (
            checkUpdates && <span className="text-fg-muted text-xs">{t("upToDate")}</span>
          )}
        </Row>
        <Row title={t("license")}>
          <span className="text-fg-muted text-xs">Apache-2.0</span>
        </Row>
        <Row title={t("privacy")} description={t("privacyNote")}>
          {null}
        </Row>
      </Card>
    </Section>
  );
}
