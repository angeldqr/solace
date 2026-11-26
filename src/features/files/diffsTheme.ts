import { type Settings, terminalFontFamily } from "../settings/settings";

// Pane colour follows the app theme; added/removed lines are tinted from it.
export const UNSAFE_CSS = `:host, pre, code { --diffs-bg: var(--color-pane); }
:host {
  --diffs-bg-addition-override: color-mix(in lab, var(--diffs-bg) 55%, var(--diffs-addition-base));
  --diffs-bg-deletion-override: color-mix(in lab, var(--diffs-bg) 55%, var(--diffs-deletion-base));
  --diffs-bg-addition-emphasis-override: rgb(from var(--diffs-addition-base) r g b / 0.12);
  --diffs-bg-deletion-emphasis-override: rgb(from var(--diffs-deletion-base) r g b / 0.12);
}`;

/** Font variables for @pierre/diffs components, from the user's terminal font settings. */
export function diffsStyle(settings: Settings) {
  const size = Math.max(11, settings.fontSize - 1);
  return {
    "--diffs-font-family": terminalFontFamily(settings.fontFamily),
    "--diffs-font-size": `${size}px`,
    "--diffs-line-height": `${Math.round(size * 1.55)}px`,
  } as React.CSSProperties;
}
