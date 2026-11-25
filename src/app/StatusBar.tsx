import { Bot, GitBranch, Monitor } from "lucide-react";
import { PALETTE_KEYS, togglePalette } from "../features/commands/commands";
import { t } from "../shared/i18n";
import { activeSpace, useSession } from "../shared/session";

export function StatusBar() {
  const view = useSession((s) => s.view);
  const space = activeSpace(view);
  const branch = space && view?.branches[space.id];
  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-line border-t px-3 text-2xs text-fg-subtle [&_svg]:size-3.5">
      <span className="flex items-center gap-1.5">
        <Monitor aria-hidden /> {t("thisComputer")}
      </span>
      <span className="flex items-center gap-1.5">
        <GitBranch aria-hidden /> {branch ?? t("noRepository")}
      </span>
      <span className="flex items-center gap-1.5">
        <Bot aria-hidden /> {t("agentCount", { n: 0 })}
      </span>
      <button
        type="button"
        onClick={togglePalette}
        className="ml-auto transition-colors hover:text-fg"
      >
        {PALETTE_KEYS} {t("commands").toLowerCase()}
      </button>
    </footer>
  );
}
