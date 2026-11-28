import { useEffect, useState } from "react";
import { type MessageKey, t } from "../../../shared/i18n";
import { api } from "../../../shared/ipc";
import { AgentIcon } from "../../../shared/ui/AgentIcon";
import { Switch } from "../../../shared/ui/Switch";
import { Card, Hint, Row, Section } from "../layout";

/** The CLIs Solace integrates with, in the order shown. */
const INTEGRATIONS: { id: string; name: string }[] = [
  { id: "claude", name: "Claude Code" },
  { id: "codex", name: "Codex" },
  { id: "antigravity", name: "Antigravity CLI" },
  { id: "kimi", name: "Kimi Code" },
  { id: "opencode", name: "OpenCode" },
];

export function Agents() {
  const [installed, setInstalled] = useState<string[] | null>(null);
  const [onPath, setOnPath] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  useEffect(() => {
    api.integrationsInstalled().then(setInstalled, (e) => setError(String(e)));
    api.agentsInstalled().then(
      (l) => setOnPath(l.map(([id]) => id)),
      () => {},
    );
  }, []);
  const toggle = (agent: string, enabled: boolean) => {
    setError(null);
    setBusy(agent);
    api
      .integrationSet(agent, enabled)
      .then(
        () =>
          setInstalled((l) =>
            enabled ? [...(l ?? []), agent] : (l ?? []).filter((a) => a !== agent),
          ),
        (e) => setError(String(e)),
      )
      .finally(() => setBusy(null));
  };
  return (
    <Section title={t("agents")}>
      <p className="mb-3 text-fg-muted text-sm">{t("integrationsIntro")}</p>
      <Card>
        {INTEGRATIONS.map(({ id, name }) => (
          <Row
            key={id}
            title={
              <span className="flex items-center gap-2">
                <AgentIcon agent={id} className="size-4" />
                {name}
                {!onPath.includes(id) && (
                  <span className="text-fg-subtle text-xs">· {t("integrationNotFound")}</span>
                )}
              </span>
            }
            description={t(`integration_${id}` as MessageKey)}
          >
            <Switch
              checked={installed?.includes(id) ?? false}
              disabled={installed === null || busy !== null}
              label={t("integrationOf", { name })}
              onCheckedChange={(on) => toggle(id, on)}
            />
          </Row>
        ))}
      </Card>
      {installed?.includes("codex") && <Hint>{t("integrationCodexTrust")}</Hint>}
      {error && <Hint tone="danger">{error}</Hint>}
    </Section>
  );
}
