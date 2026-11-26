import { i18n } from "@pierre/diffs";
import { PatchDiff } from "@pierre/diffs/react";
import { useMemo } from "react";
import { locale, t } from "../../shared/i18n";
import { resolveTheme, useSettings, useSystemDark } from "../settings/settings";
import { diffsStyle, UNSAFE_CSS } from "./diffsTheme";

// The library renders its own hunk separators; "#" is its plural placeholder. The language only changes on reload.
i18n.set({
  locale,
  messages: { unmodifiedLines: `{count, plural, other {${t("unmodifiedLines", { n: "#" })}}}` },
});

/** Loaded lazily: Shiki and its grammars stay out of the main chunk until a diff opens. */
export default function PatchViewer({ patch, split }: { patch: string; split: boolean }) {
  const settings = useSettings();
  const systemDark = useSystemDark();
  const dark = resolveTheme(settings, systemDark).dark;
  const wrap = settings.wordWrap;

  const options = useMemo(
    () => ({
      diffStyle: split ? ("split" as const) : ("unified" as const),
      theme: { dark: "pierre-dark", light: "pierre-light" },
      themeType: dark ? ("dark" as const) : ("light" as const),
      // DiffView shows the file name itself.
      disableFileHeader: true,
      tokenizeMaxLineLength: 1000,
      disableLineNumbers: false,
      overflow: wrap ? ("wrap" as const) : ("scroll" as const),
      // The JS regex engine: no WASM, so the CSP needs no 'wasm-unsafe-eval'.
      preferredHighlighter: "shiki-js" as const,
      unsafeCSS: UNSAFE_CSS,
    }),
    [split, dark, wrap],
  );

  // shortcut: no virtualization/worker pool; "Show anyway" on huge diffs renders every line on the main thread. Add @pierre/diffs Virtualizer if that becomes a problem.
  return (
    <div className="min-h-0 flex-1 overflow-auto select-text" style={diffsStyle(settings)}>
      <PatchDiff patch={patch} options={options} disableWorkerPool />
    </div>
  );
}
