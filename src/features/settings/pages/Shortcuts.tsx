import { type MessageKey, t } from "../../../shared/i18n";
import { Kbd } from "../../../shared/ui/Kbd";
import { commands, PALETTE_KEYS } from "../../commands/commands";
import { Card, Row, Section } from "../layout";

const GROUPS: MessageKey[] = ["groupTerminals", "groupLayout", "groupPanels", "groupApp"];

export function Shortcuts() {
  return (
    <Section title={t("keyboardShortcuts")}>
      <Card>
        <Row title={t("commands")}>
          <Kbd keys={PALETTE_KEYS} />
        </Row>
        {GROUPS.map((group) => (
          <div key={group}>
            <h4 className="px-4 pt-3 font-medium text-fg-muted text-xs">{t(group)}</h4>
            {commands
              .filter((c) => c.group === group && c.keys)
              .map((command) => (
                <Row key={command.id} title={t(command.title)}>
                  <Kbd keys={command.keys ?? ""} />
                </Row>
              ))}
          </div>
        ))}
      </Card>
    </Section>
  );
}
