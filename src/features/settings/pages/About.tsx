import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { t } from "../../../shared/i18n";
import { Button } from "../../../shared/ui/Button";
import { checkNow, restartToUpdate, useUpdate } from "../../updates/updates";
import { Card, Row, Section } from "../layout";

type Check = "idle" | "checking" | "upToDate" | "ready" | "failed";

export function About() {
  const [version, setVersion] = useState("");
  const [check, setCheck] = useState<Check>("idle");
  const update = useUpdate((s) => s.ready);
  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);
  const search = async () => {
    setCheck("checking");
    setCheck(await checkNow());
  };
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
            <>
              {(check === "upToDate" || check === "failed") && (
                <span
                  role="status"
                  className={`text-xs ${check === "failed" ? "text-danger" : "text-fg-muted"}`}
                >
                  {t(check === "failed" ? "updateCheckFailed" : "upToDate")}
                </span>
              )}
              <Button size="sm" disabled={check === "checking"} onClick={() => void search()}>
                {t(check === "checking" ? "checkingUpdates" : "checkForUpdates")}
              </Button>
            </>
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
