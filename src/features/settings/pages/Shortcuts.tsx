import { t } from "../../../shared/i18n";
import { Kbd } from "../../../shared/ui/Kbd";
import { commands, PALETTE_KEYS } from "../../commands/commands";
import { Card, Row, Section } from "../layout";

export function Shortcuts() {
  return (
    <Section title={t("keyboardShortcuts")}>
      <Card>
        <Row title={t("commands")}>
          <Kbd keys={PALETTE_KEYS} />
        </Row>
        {commands.map(
          (command) =>
            command.keys && (
              <Row key={command.id} title={t(command.title)}>
                <Kbd keys={command.keys} />
              </Row>
            ),
        )}
      </Card>
    </Section>
  );
}
