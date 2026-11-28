import { type Appearance, useSettings } from "./settings";
import { FAMILIES, type Theme } from "./themes";

/** The light and dark variants of a family; an imported theme has only one. */
function variants(familyId: string, custom: Theme | null): { light: Theme; dark: Theme } {
  if (familyId === "custom" && custom) return { light: custom, dark: custom };
  const family = FAMILIES.find((f) => f.id === familyId) ?? FAMILIES[0];
  return { light: family.light ?? family.dark, dark: family.dark };
}

const bar = (color: string, width: string, opacity = 1) => (
  <span className="block h-1 rounded-full" style={{ background: color, width, opacity }} />
);

/** A decorative miniature of the app: sidebar, two terminal panes and a side panel. */
function MiniSolace({ theme }: { theme: Theme }) {
  const { ui, terminal: term } = theme;
  const pane = (
    <span
      className="flex flex-1 flex-col gap-1 rounded-md p-1.5"
      style={{ background: term.background }}
    >
      {bar(term.foreground ?? ui.fg, "70%", 0.8)}
      {bar(term.green ?? ui.primary, "45%")}
      {bar(term.blue ?? ui.primary, "60%")}
    </span>
  );
  return (
    <span
      aria-hidden
      className="relative flex aspect-[16/10] w-full overflow-hidden rounded-lg border border-line"
      style={{ background: ui.canvas }}
    >
      <span className="flex w-[22%] flex-col gap-1.5 p-2">
        {bar(ui.primary, "80%")}
        {bar(ui["fg-subtle"], "60%")}
        {bar(ui["fg-subtle"], "70%")}
      </span>
      <span className="flex flex-1 flex-col gap-1 py-1.5">
        <span className="h-1.5 rounded-full" style={{ background: ui.surface }} />
        <span className="flex flex-1 gap-1">
          {pane}
          {pane}
        </span>
      </span>
      <span className="ml-1.5 flex w-[18%] flex-col gap-1.5 p-2" style={{ background: ui.surface }}>
        {bar(ui["fg-subtle"], "80%")}
        {bar(ui["fg-subtle"], "55%")}
        {bar(ui["fg-subtle"], "70%")}
        {bar(ui["fg-subtle"], "45%")}
      </span>
    </span>
  );
}

/** Light, dark, or both halves side by side for "system". */
export function ModePreview({ mode, familyId }: { mode: Appearance; familyId: string }) {
  const custom = useSettings((s) => s.custom);
  const { light, dark } = variants(familyId, custom);
  if (mode !== "system") return <MiniSolace theme={mode === "dark" ? dark : light} />;
  return (
    <span className="relative block">
      <MiniSolace theme={light} />
      <span className="absolute inset-0 block" style={{ clipPath: "inset(0 0 0 50%)" }}>
        <MiniSolace theme={dark} />
      </span>
      <span aria-hidden className="absolute inset-y-0 left-1/2 w-px bg-line-strong" />
    </span>
  );
}

export function ThemeOrb({ theme, size = 44 }: { theme: Theme; size?: number }) {
  const { primary, canvas } = theme.ui;
  return (
    <span
      aria-hidden
      className="block shrink-0 rounded-full ring-1 ring-line"
      style={{
        width: size,
        height: size,
        background: `radial-gradient(circle at 32% 28%, color-mix(in oklab, ${primary} 55%, white) 0%, ${primary} 38%, ${canvas} 78%)`,
      }}
    />
  );
}
