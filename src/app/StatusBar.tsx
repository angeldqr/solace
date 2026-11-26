import { ArrowUpCircle, Bot, Command, GitBranch, Monitor } from "lucide-react";
import { PALETTE_KEYS, togglePalette } from "../features/commands/commands";
import { restartToUpdate, useUpdate } from "../features/updates/updates";
import { useAgents } from "../shared/agents";
import { t } from "../shared/i18n";
import { activeSpace, useSession } from "../shared/session";
import { Kbd } from "../shared/ui/Kbd";
import { Tooltip } from "../shared/ui/Tooltip";

export function StatusBar() {
  const view = useSession((s) => s.view);
  const space = activeSpace(view);
  const branch = space && view?.branches[space.id];
  const agentCount = useAgents((s) => s.list.length);
  const update = useUpdate((s) => s.ready);
  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-line border-t px-3 text-fg-subtle text-xs [&_svg]:size-3.5">
      <span className="flex items-center gap-1.5">
        <Monitor aria-hidden /> {t("thisComputer")}
      </span>
      <span className="flex items-center gap-1.5">
        <GitBranch aria-hidden /> {branch ?? t("noRepository")}
      </span>
      <span className="flex items-center gap-1.5">
        <Bot aria-hidden /> {t("agentCount", { n: agentCount })}
      </span>
      {update && (
        <Tooltip label={t("updateHint")}>
          <button
            type="button"
            onClick={() => void restartToUpdate()}
            className="ml-auto flex items-center gap-1.5 rounded-md px-1.5 text-primary-soft transition-colors hover:bg-hover hover:text-fg"
          >
            <ArrowUpCircle aria-hidden /> {t("restartToUpdate", { version: update.version })}
          </button>
        </Tooltip>
      )}
      <button
        type="button"
        onClick={togglePalette}
        className={`${update ? "" : "ml-auto "}flex items-center gap-1.5 rounded-md px-1.5 transition-colors hover:bg-hover hover:text-fg`}
      >
        <Command aria-hidden />
        <Kbd keys={PALETTE_KEYS} /> {t("commands").toLowerCase()}
      </button>
    </footer>
  );
}
