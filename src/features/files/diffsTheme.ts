import { fontStack, type Settings } from "../settings/settings";

// Pane colour follows the app theme; added/removed lines are tinted from it.
export const UNSAFE_CSS = `:host, pre, code { --diffs-bg: var(--color-pane); }
:host {
  --diffs-bg-addition-override: color-mix(in lab, var(--diffs-bg) 55%, var(--diffs-addition-base));
  --diffs-bg-deletion-override: color-mix(in lab, var(--diffs-bg) 55%, var(--diffs-deletion-base));
  --diffs-bg-addition-emphasis-override: rgb(from var(--diffs-addition-base) r g b / 0.12);
  --diffs-bg-deletion-emphasis-override: rgb(from var(--diffs-deletion-base) r g b / 0.12);
}`;

/** Font and colour variables for @pierre/diffs components, from the code settings. They inherit
 * into the library's shadow DOM, where the base colours read the *-color-override variables. */
export function diffsStyle(settings: Settings) {
  const size = settings.codeFontSize;
  return {
    "--diffs-font-family": fontStack(settings.codeFont, "mono"),
    "--diffs-font-size": `${size}px`,
    "--diffs-line-height": `${Math.round(size * 1.55)}px`,
    ...(settings.diffColors === "blue-orange" && {
      "--diffs-addition-color-override": "#3b82f6",
      "--diffs-deletion-color-override": "#f97316",
    }),
  } as React.CSSProperties;
}
