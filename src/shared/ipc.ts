import { type Channel, invoke } from "@tauri-apps/api/core";

export type SpaceId = number;
export type TabId = number;
export type PaneId = number;
export type Direction = "row" | "column";

export type Layout =
  | { type: "leaf"; pane: PaneId }
  | { type: "split"; direction: Direction; ratio: number; first: Layout; second: Layout };

export interface Tab {
  id: TabId;
  layout: Layout;
  focused: PaneId;
}

export interface Space {
  id: SpaceId;
  name: string;
  root: string;
  tabs: Tab[];
  activeTab: TabId | null;
}

export interface SessionView {
  spaces: Space[];
  activeSpace: SpaceId | null;
  /** Checked-out branch per space id; absent when the folder is not a git repository. */
  branches: Record<string, string>;
}

export type FileContent =
  | { kind: "text"; text: string; size: number }
  | { kind: "image"; dataUrl: string; size: number }
  | { kind: "binary"; size: number }
  | { kind: "tooLarge"; size: number };

export interface FileEntry {
  name: string;
  dir: boolean;
  ignored: boolean;
}

export interface Change {
  path: string;
  /** M modified, A added, D deleted, R renamed, U untracked, C copied, T type change. */
  status: string;
  /** The path before a rename. */
  from: string | null;
  additions: number | null;
  deletions: number | null;
}

export interface Shell {
  name: string;
  path: string;
}

/** Typed wrappers over the Rust commands in src-tauri/src/commands.rs. */
export const api = {
  sessionGet: () => invoke<SessionView>("session_get"),
  spaceOpen: (path: string) => invoke("space_open", { path }),
  spaceClose: (space: SpaceId) => invoke("space_close", { space }),
  spaceActivate: (space: SpaceId) => invoke("space_activate", { space }),
  tabNew: (space: SpaceId) => invoke("tab_new", { space }),
  tabNewAgent: (space: SpaceId, kind: string) => invoke("tab_new_agent", { space, kind }),
  /** Agents found on PATH, as [id, name]. */
  agentsInstalled: () => invoke<[string, string][]>("agents_installed"),
  tabActivate: (tab: TabId) => invoke("tab_activate", { tab }),
  tabClose: (tab: TabId) => invoke("tab_close", { tab }),
  paneSplit: (pane: PaneId, direction: Direction) => invoke("pane_split", { pane, direction }),
  paneClose: (pane: PaneId) => invoke("pane_close", { pane }),
  paneFocus: (pane: PaneId) => invoke("pane_focus", { pane }),
  splitResize: (tab: TabId, path: number[], ratio: number) =>
    invoke("split_resize", { tab, path, ratio }),
  ptyAttach: (pane: PaneId, cols: number, rows: number, output: Channel<ArrayBuffer>) =>
    invoke("pty_attach", { pane, cols, rows, output }),
  /** False when the pane's shell has ended. */
  ptyWrite: (pane: PaneId, data: string) => invoke<boolean>("pty_write", { pane, data }),
  ptyResize: (pane: PaneId, cols: number, rows: number) =>
    invoke("pty_resize", { pane, cols, rows }),
  ptyPause: (pane: PaneId, paused: boolean) => invoke("pty_pause", { pane, paused }),
  filesList: (space: SpaceId, path: string) => invoke<FileEntry[]>("files_list", { space, path }),
  gitStatus: (space: SpaceId) => invoke<Change[] | null>("git_status", { space }),
  gitDiff: (
    space: SpaceId,
    path: string,
    from: string | null,
    untracked: boolean,
    ignoreWhitespace: boolean,
  ) => invoke<string>("git_diff", { space, path, from, untracked, ignoreWhitespace }),
  gitShowImage: (space: SpaceId, path: string) =>
    invoke<string | null>("git_show_image", { space, path }),
  fileOpen: (space: SpaceId, path: string) => invoke("file_open", { space, path }),
  openUrl: (url: string) => invoke("open_url", { url }),
  fileRead: (space: SpaceId, path: string) => invoke<FileContent>("file_read", { space, path }),
  shellsList: () => invoke<Shell[]>("shells_list"),
  settingsGet: () => invoke<{ shell: string | null; notifications: boolean }>("settings_get"),
  notificationsSet: (enabled: boolean) => invoke("notifications_set", { enabled }),
  localeSet: (locale: string) => invoke("locale_set", { locale }),
  systemToastsEnabled: () => invoke<boolean>("system_toasts_enabled"),
  claudeIntegrationInstalled: () => invoke<boolean>("claude_integration_installed"),
  claudeIntegrationSet: (enabled: boolean) => invoke("claude_integration_set", { enabled }),
  usageGet: () => invoke<UsageView>("usage_get"),
  presenceAvailable: () => invoke<boolean>("presence_available"),
  presenceStatus: () => invoke<number>("presence_status"),
  presenceUpdate: (activity: PresenceActivity | null) => invoke("presence_update", { activity }),
  shellSet: (path: string | null) => invoke("shell_set", { path }),
  tasksGet: () => invoke<BatchView[]>("tasks_get"),
  /** Rejects with git's reason when a worktree still holds uncommitted work. */
  tasksClean: (batch: string) => invoke("tasks_clean", { batch }),
};

export type TaskStatus = "starting" | "running" | "done" | "failed" | "closed";

export interface TaskView {
  id: string;
  title: string;
  /** Catalog id of the worker's agent, e.g. "codex". */
  kind: string;
  /** Null when the worker's pane could not be opened. */
  pane: PaneId | null;
  branch: string | null;
  worktree: string | null;
  /** Commit the worktree started from; null in a shared folder. */
  base: string | null;
  status: TaskStatus;
  /** What the agent in the pane is doing now; null when none runs there. */
  agentState: "idle" | "working" | "blocked" | null;
  summary: string | null;
  error: string | null;
  /** Unix ms of the last status change. */
  since: number;
}

/** What one `solace tasks start` launched. */
export interface BatchView {
  id: string;
  lead: PaneId | null;
  repo: string | null;
  base: string | null;
  createdAt: number;
  tasks: TaskView[];
}

export interface PresenceActivity {
  details: string;
  state?: string;
  smallImage?: string;
  smallText?: string;
}

export interface Limit {
  windowMinutes: number;
  usedPercent: number;
  /** Unix seconds. */
  resetsAt: number | null;
}

export interface ProviderUsage {
  limits: Limit[];
  plan: string | null;
  /** Unix ms of the last report. */
  updatedAt: number;
}

export interface Tokens {
  input: number;
  output: number;
  cacheCreation: number;
  cacheRead: number;
}

export interface UsageView {
  claude: ProviderUsage | null;
  codex: ProviderUsage | null;
  sessions: {
    pane: PaneId;
    agent: string;
    model: string | null;
    usedTokens: number | null;
    limit: number | null;
    percent: number | null;
    costUsd: number | null;
  }[];
  tokens: {
    /** Unix seconds. */
    blockStart: number;
    block: Tokens;
    hourly: { hour: number; tokens: Tokens }[];
  };
}
