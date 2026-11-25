import { ArrowUpCircle, Bot, GitBranch, Monitor } from "lucide-react";
import { PALETTE_KEYS, togglePalette } from "../features/commands/commands";
import { restartToUpdate, useUpdate } from "../features/updates/updates";
import { useAgents } from "../shared/agents";
import { t } from "../shared/i18n";
import { activeSpace, useSession } from "../shared/session";

export function StatusBar() {
  const view = useSession((s) => s.view);
  const space = activeSpace(view);
  const branch = space && view?.branches[space.id];
  const agentCount = useAgents((s) => s.list.length);
  const update = useUpdate((s) => s.ready);
  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-line border-t px-3 text-2xs text-fg-subtle [&_svg]:size-3.5">
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
        <button
          type="button"
          onClick={() => void restartToUpdate()}
          title={t("updateHint")}
          className="ml-auto flex items-center gap-1.5 rounded px-1.5 text-primary-soft transition-colors hover:bg-hover hover:text-fg"
        >
          <ArrowUpCircle aria-hidden /> {t("restartToUpdate", { version: update.version })}
        </button>
      )}
      <button
        type="button"
        onClick={togglePalette}
        className={`${update ? "" : "ml-auto "}transition-colors hover:text-fg`}
      >
        {PALETTE_KEYS} {t("commands").toLowerCase()}
      </button>
    </footer>
  );
}
