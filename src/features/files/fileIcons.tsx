import { createFileTreeIconResolver, getBuiltInSpriteSheet } from "@pierre/trees";
import { Folder, FolderOpen } from "lucide-react";

const SET = "complete";
const resolver = createFileTreeIconResolver({ set: SET, colored: true });

/** The library's icons are `currentColor`; these mid-tones read on both light and dark panes. */
const COLORS: Record<string, string> = {
  typescript: "#3b82f6",
  javascript: "#ca8a04",
  babel: "#ca8a04",
  react: "#38bdf8",
  json: "#d97706",
  markdown: "#22c55e",
  rust: "#f97316",
  css: "#a855f7",
  sass: "#ec4899",
  postcss: "#ef4444",
  tailwind: "#06b6d4",
  html: "#f97316",
  image: "#ec4899",
  svg: "#ec4899",
  python: "#3b82f6",
  go: "#06b6d4",
  vue: "#22c55e",
  svelte: "#f97316",
  yml: "#ef4444",
  docker: "#3b82f6",
  git: "#f97316",
  npm: "#ef4444",
  bash: "#22c55e",
  vite: "#a855f7",
};

let injected = false;
/** The sprite holds every symbol once; icons reference it with <use>. */
function injectSprite() {
  if (injected) return;
  injected = true;
  const host = document.createElement("div");
  host.setAttribute("aria-hidden", "true");
  host.style.cssText = "position:absolute;width:0;height:0;overflow:hidden";
  host.innerHTML = getBuiltInSpriteSheet(SET);
  document.body.append(host);
}

export function FileIcon({ name }: { name: string }) {
  injectSprite();
  const icon = resolver.resolveIcon("file-tree-icon-file", name);
  const color = icon.token ? COLORS[icon.token] : undefined;
  return (
    <svg
      aria-hidden
      viewBox={icon.viewBox ?? "0 0 16 16"}
      className="size-4 shrink-0 text-fg-subtle"
      style={color ? { color } : undefined}
    >
      <use href={`#${icon.name}`} />
    </svg>
  );
}

export function FolderIcon({ open }: { open: boolean }) {
  const Icon = open ? FolderOpen : Folder;
  return (
    <Icon aria-hidden className={`size-4 shrink-0 ${open ? "text-fg-muted" : "text-fg-subtle"}`} />
  );
}
