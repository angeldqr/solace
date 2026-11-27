import amp from "./agents/amp-color.svg";
import antigravity from "./agents/antigravity.png";
import claude from "./agents/claudecode-color.svg";
import codex from "./agents/codex-color.svg";
import cursor from "./agents/cursor.svg";
import copilot from "./agents/githubcopilot.svg";
import kimi from "./agents/kimi-color.svg";
import opencode from "./agents/opencode.svg";
import qwen from "./agents/qwen-color.svg";

/** Brand marks from Lobe Icons (MIT). Monochrome ones are black and get inverted on dark UI. */
export const agentIcons: Record<string, { src: string; mono?: boolean }> = {
  claude: { src: claude },
  codex: { src: codex },
  antigravity: { src: antigravity },
  qwen: { src: qwen },
  kimi: { src: kimi },
  opencode: { src: opencode, mono: true },
  copilot: { src: copilot, mono: true },
  cursor: { src: cursor, mono: true },
  amp: { src: amp },
};
