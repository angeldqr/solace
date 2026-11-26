import { useEffect, useState } from "react";
import { t } from "../../../shared/i18n";
import { api, type Shell as ShellInfo } from "../../../shared/ipc";
import { Select } from "../../../shared/ui/Select";
import { Card, Hint, Row, Section } from "../layout";

export function Shell() {
  const [shells, setShells] = useState<ShellInfo[]>([]);
  const [selected, setSelected] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api.shellsList().then(setShells);
    void api.settingsGet().then((settings) => setSelected(settings.shell ?? ""));
  }, []);

  const choose = (path: string) => {
    setSelected(path);
    api.shellSet(path || null).then(
      () => setError(null),
      (e) => setError(String(e)),
    );
  };

  const options = [
    { value: "", label: t("shellDefault") },
    ...shells.map((shell) => ({ value: shell.path, label: shell.name })),
  ];
  return (
    <Section title={t("shell")}>
      <Card>
        <Row title={t("shellForNewTerminals")}>
          <Select
            value={selected}
            options={options}
            label={t("shellForNewTerminals")}
            onChange={choose}
          />
        </Row>
      </Card>
      {error && <Hint tone="danger">{error}</Hint>}
    </Section>
  );
}
