import { PatchDiff } from "@pierre/diffs/react";
import { useMemo } from "react";
import { resolveTheme, terminalFontFamily, useSettings, useSystemDark } from "../settings/settings";

// Pane colour follows the app theme; added/removed lines are tinted from it.
const UNSAFE_CSS = `:host, pre, code { --diffs-bg: var(--color-pane); }
:host {
  --diffs-bg-addition-override: color-mix(in lab, var(--diffs-bg) 55%, var(--diffs-addition-base));
  --diffs-bg-deletion-override: color-mix(in lab, var(--diffs-bg) 55%, var(--diffs-deletion-base));
  --diffs-bg-addition-emphasis-override: rgb(from var(--diffs-addition-base) r g b / 0.12);
  --diffs-bg-deletion-emphasis-override: rgb(from var(--diffs-deletion-base) r g b / 0.12);
}`;

/** Loaded lazily: Shiki and its grammars stay out of the main chunk until a diff opens. */
export default function PatchViewer({ patch, split }: { patch: string; split: boolean }) {
  const settings = useSettings();
  const systemDark = useSystemDark();
  const dark = resolveTheme(settings, systemDark).dark;

  const options = useMemo(
    () => ({
      diffStyle: split ? ("split" as const) : ("unified" as const),
      theme: { dark: "pierre-dark", light: "pierre-light" },
      themeType: dark ? ("dark" as const) : ("light" as const),
      // DiffView shows the file name itself.
      disableFileHeader: true,
      tokenizeMaxLineLength: 1000,
      disableLineNumbers: false,
      // The JS regex engine: no WASM, so the CSP needs no 'wasm-unsafe-eval'.
      preferredHighlighter: "shiki-js" as const,
      unsafeCSS: UNSAFE_CSS,
    }),
    [split, dark],
  );

  // shortcut: no virtualization/worker pool; "Show anyway" on huge diffs renders every line on the main thread. Add @pierre/diffs Virtualizer if that becomes a problem.
  const size = Math.max(11, settings.fontSize - 1);
  return (
    <div
      className="min-h-0 flex-1 overflow-auto select-text"
      style={
        {
          "--diffs-font-family": terminalFontFamily(settings.fontFamily),
          "--diffs-font-size": `${size}px`,
          "--diffs-line-height": `${Math.round(size * 1.55)}px`,
        } as React.CSSProperties
      }
    >
      <PatchDiff patch={patch} options={options} disableWorkerPool />
    </div>
  );
}
