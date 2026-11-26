import { File } from "@pierre/diffs/react";
import { useMemo } from "react";
import { resolveTheme, useSettings, useSystemDark } from "../settings/settings";
import { diffsStyle, UNSAFE_CSS } from "./diffsTheme";

/** Loaded lazily, like PatchViewer: Shiki stays out of the main chunk until a file opens. */
export default function CodeViewer({ name, contents }: { name: string; contents: string }) {
  const settings = useSettings();
  const wrap = settings.wordWrap;
  const dark = resolveTheme(settings, useSystemDark()).dark;
  const file = useMemo(() => ({ name, contents }), [name, contents]);
  const options = useMemo(
    () => ({
      theme: { dark: "pierre-dark", light: "pierre-light" },
      themeType: dark ? ("dark" as const) : ("light" as const),
      disableFileHeader: true,
      tokenizeMaxLineLength: 1000,
      overflow: wrap ? ("wrap" as const) : ("scroll" as const),
      preferredHighlighter: "shiki-js" as const,
      unsafeCSS: UNSAFE_CSS,
    }),
    [dark, wrap],
  );
  return (
    <div className="min-h-0 flex-1 overflow-auto select-text" style={diffsStyle(settings)}>
      <File file={file} options={options} disableWorkerPool />
    </div>
  );
}
