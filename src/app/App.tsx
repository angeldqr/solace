import { FolderOpen, GitCompare, PanelRight, SquareTerminal } from "lucide-react";
import { useEffect } from "react";
import { CommandPalette } from "../features/commands/CommandPalette";
import {
  commandFor,
  isAppShortcut,
  openFolder,
  PALETTE_KEYS,
  runCommand,
  togglePalette,
} from "../features/commands/commands";
import { togglePanel, usePanel } from "../features/files/panel";
import { RightPanel } from "../features/files/RightPanel";
import { SettingsDialog } from "../features/settings/SettingsDialog";
import { Sidebar } from "../features/spaces/Sidebar";
import { PaneTree } from "../features/terminals/PaneTree";
import { setKeyInterceptor } from "../features/terminals/registry";
import { TabBar } from "../features/terminals/TabBar";
import { t } from "../shared/i18n";
import { api, type Space } from "../shared/ipc";
import { matches } from "../shared/keys";
import { activeSpace, activeTab, useSession } from "../shared/session";
import { StatusBar } from "./StatusBar";
import { WindowControls } from "./WindowControls";

setKeyInterceptor(isAppShortcut);

function useShortcuts() {
  useEffect(() => {
    // Capture phase: runs before the focused terminal sees the key.
    const onKeyDown = (event: KeyboardEvent) => {
      // Outside a terminal, browser reload keys would wipe every scrollback (xterm consumes them itself).
      const inTerminal = event.target instanceof Element && event.target.closest(".xterm");
      if (
        !inTerminal &&
        (event.key === "F5" || (event.ctrlKey && event.key.toLowerCase() === "r"))
      ) {
        event.preventDefault();
        return;
      }
      // Holding a shortcut must not open or close a pane per auto-repeat.
      if (event.repeat && isAppShortcut(event) && !commandFor(event)?.repeat) {
        event.preventDefault();
        return;
      }
      if (matches(event, PALETTE_KEYS)) {
        event.preventDefault();
        togglePalette();
        return;
      }
      const command = commandFor(event);
      if (command) {
        event.preventDefault();
        runCommand(command);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);
}

export function App() {
  const view = useSession((s) => s.view);
  useShortcuts();
  const space = activeSpace(view);

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <header data-tauri-drag-region className="flex h-11 shrink-0 items-center">
            {space && <TabBar space={space} />}
            <div data-tauri-drag-region className="h-full flex-1" />
            {space && <PanelToggles />}
            <WindowControls />
          </header>
          <div className="flex min-h-0 flex-1">
            <main className="min-w-0 flex-1 px-2 pb-2">{view && <Workspace space={space} />}</main>
            {space && <RightPanel space={space} />}
          </div>
        </div>
      </div>
      <StatusBar />
      <CommandPalette />
      <SettingsDialog />
    </div>
  );
}

/** Herdr-style switches for the right panel, next to the window controls. */
function PanelToggles() {
  const { open, tab } = usePanel();
  const button = (which: "files" | "changes", label: string, icon: React.ReactNode) => {
    const active = open && tab === which;
    return (
      <button
        type="button"
        aria-pressed={active}
        onClick={() => togglePanel(which)}
        className={`flex h-7 items-center gap-1.5 rounded-md px-2 text-xs transition-colors duration-150 [&>svg]:size-3.5 ${
          active ? "bg-selected text-fg" : "text-fg-muted hover:bg-hover hover:text-fg"
        }`}
      >
        {icon}
        {label}
      </button>
    );
  };
  return (
    <div className="mr-2 flex items-center gap-1">
      {button("files", t("files"), <PanelRight aria-hidden />)}
      {button("changes", t("changes"), <GitCompare aria-hidden />)}
    </div>
  );
}

function Workspace({ space }: { space?: Space }) {
  if (!space) {
    return (
      <EmptyState
        title={t("startTitle")}
        body={t("startBody")}
        action={t("openFolder")}
        keys="Ctrl+Shift+O"
        icon={<FolderOpen aria-hidden />}
        onAction={openFolder}
      />
    );
  }
  const tab = activeTab(space);
  if (!tab) {
    return (
      <EmptyState
        title={t("noTerminals")}
        action={t("newTerminal")}
        keys="Ctrl+Shift+T"
        icon={<SquareTerminal aria-hidden />}
        onAction={() => api.tabNew(space.id)}
      />
    );
  }
  return <PaneTree key={tab.id} space={space} tab={tab} />;
}

function EmptyState(props: {
  title: string;
  body?: string;
  action: string;
  keys: string;
  icon: React.ReactNode;
  onAction: () => unknown;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-5 pb-11 text-center">
      <img src="/solace.svg" alt="" className="size-14" />
      <div className="space-y-1.5">
        <h1 className="font-medium text-lg text-fg tracking-tight">{props.title}</h1>
        {props.body && <p className="max-w-sm text-fg-muted leading-relaxed">{props.body}</p>}
      </div>
      <button
        type="button"
        onClick={() => void props.onAction()}
        className="flex h-8 items-center gap-2 rounded-md bg-primary px-3 font-medium text-on-primary transition-[filter] duration-150 hover:brightness-110 [&>svg]:size-3.5"
      >
        {props.icon}
        {props.action}
        <kbd className="ml-1 font-sans text-2xs text-on-primary/60">{props.keys}</kbd>
      </button>
    </div>
  );
}
