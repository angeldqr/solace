import { Channel } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import { api, type PaneId } from "../../shared/ipc";
import { panesOf, setTitle, useSession } from "../../shared/session";
import { activeTheme, terminalFontFamily, useSettings, useSystemDark } from "../settings/settings";

// xterm.js flow-control guide: pause the pty above HIGH unprocessed bytes; HIGH under ~500K keeps typing snappy.
const HIGH = 256 * 1024;
const LOW = 32 * 1024;

/** Options from the user's settings, for new terminals and live changes. */
function terminalOptions() {
  const s = useSettings.getState();
  return {
    fontFamily: terminalFontFamily(s.fontFamily),
    fontSize: s.fontSize,
    lineHeight: s.lineHeight,
    cursorStyle: s.cursorStyle,
    cursorBlink: s.cursorBlink,
    theme: activeTheme(s).terminal,
  };
}

/** The WebGL glyph atlas is measured on open, so the font must be loaded first. */
function fontReady() {
  const { fontFamily, fontSize } = useSettings.getState();
  return document.fonts
    .load(`${fontSize}px ${terminalFontFamily(fontFamily)}`)
    .catch(() => undefined);
}
let fontsReady = fontReady();

interface Entry {
  term: Terminal;
  fit: FitAddon;
  /** Stable element the terminal renders into; moved between hosts as tabs change. */
  el: HTMLDivElement;
  opened: boolean;
  /** False while attaching, so Enter can't start a second shell meanwhile. */
  attached: boolean;
  webgl?: WebglAddon;
  resizeTimer?: number;
}

/** Terminals live outside React so their buffers and shells survive tab switches. */
const entries = new Map<PaneId, Entry>();

let interceptKey: (event: KeyboardEvent) => boolean = () => false;
/** Pane that should hold keyboard focus once its terminal has opened. */
let wantsFocus: PaneId | undefined;

/** Lets app shortcuts reach the window instead of the shell. */
export function setKeyInterceptor(fn: (event: KeyboardEvent) => boolean) {
  interceptKey = fn;
}

function create(pane: PaneId): Entry {
  const term = new Terminal({
    ...terminalOptions(),
    scrollback: 10_000,
    allowProposedApi: true,
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new Unicode11Addon());
  term.unicode.activeVersion = "11";

  term.onData((data) => {
    void api.ptyWrite(pane, data).then((alive) => {
      if (!alive && data.includes("\r")) restart(pane);
    });
  });
  term.onTitleChange((title) => setTitle(pane, title));
  term.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown") return true;
    if (interceptKey(event)) return false;
    const key = event.key.toLowerCase();
    if (event.ctrlKey && key === "c" && term.hasSelection()) {
      void navigator.clipboard.writeText(term.getSelection());
      term.clearSelection();
      return false;
    }
    // Let the browser fire a paste event, which xterm forwards to the shell.
    if (event.ctrlKey && key === "v") return false;
    return true;
  });

  const el = document.createElement("div");
  el.className = "h-full w-full";
  return { term, fit, el, opened: false, attached: false };
}

/** A pane whose shell failed stays open showing the exit code; Enter starts a new shell there. */
function restart(pane: PaneId) {
  const entry = entries.get(pane);
  if (!entry?.attached) return;
  entry.attached = false;
  connect(pane, entry);
}

function connect(pane: PaneId, entry: Entry) {
  const { term } = entry;
  let pending = 0;
  let paused = false;
  const output = new Channel<ArrayBuffer>((buffer) => {
    const data = new Uint8Array(buffer);
    pending += data.length;
    term.write(data, () => {
      pending -= data.length;
      if (paused && pending < LOW) {
        paused = false;
        void api.ptyPause(pane, false);
      }
    });
    if (!paused && pending > HIGH) {
      paused = true;
      void api.ptyPause(pane, true);
    }
  });
  api.ptyAttach(pane, term.cols, term.rows, output).then(
    () => {
      entry.attached = true;
    },
    (error) => {
      // Lets Enter try again once the cause (a missing folder, a bad shell setting) is fixed.
      entry.attached = true;
      term.write(`\x1b[31m${String(error)}\x1b[0m\r\n`);
    },
  );
}

function enableWebgl(entry: Entry) {
  if (entry.webgl) return;
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      webgl.dispose();
      entry.webgl = undefined;
    });
    entry.term.loadAddon(webgl);
    entry.webgl = webgl;
  } catch {
    // No WebGL2: xterm keeps its DOM renderer.
  }
}

function refit(pane: PaneId, entry: Entry) {
  if (!entry.opened || !entry.el.isConnected) return;
  const dims = entry.fit.proposeDimensions();
  if (!dims || !Number.isFinite(dims.cols) || !Number.isFinite(dims.rows)) return;
  if (dims.cols === entry.term.cols && dims.rows === entry.term.rows) return;
  entry.term.resize(dims.cols, dims.rows);
  // Debounced so dragging a divider doesn't make the shell reflow on every frame.
  window.clearTimeout(entry.resizeTimer);
  entry.resizeTimer = window.setTimeout(
    () => void api.ptyResize(pane, entry.term.cols, entry.term.rows),
    50,
  );
}

/** Shows the pane's terminal inside `host`, starting its shell the first time. Returns an unmount. */
export function mountTerminal(pane: PaneId, host: HTMLElement) {
  let entry = entries.get(pane);
  if (!entry) {
    entry = create(pane);
    entries.set(pane, entry);
  }
  const current = entry;
  host.appendChild(current.el);
  let active = true;

  void fontsReady.then(() => {
    if (!active) return;
    if (!current.opened) {
      current.term.open(current.el);
      current.opened = true;
      current.fit.fit();
      connect(pane, current);
    }
    if (wantsFocus === pane) current.term.focus();
    // Hidden tabs drop their WebGL context: browsers cap live contexts at ~16.
    enableWebgl(current);
    refit(pane, current);
  });

  const observer = new ResizeObserver(() => requestAnimationFrame(() => refit(pane, current)));
  observer.observe(host);

  return () => {
    active = false;
    observer.disconnect();
    current.webgl?.dispose();
    current.webgl = undefined;
    current.el.remove();
  };
}

export function focusTerminal(pane: PaneId | undefined) {
  wantsFocus = pane;
  if (pane !== undefined) entries.get(pane)?.term.focus();
}

// Free terminals whose panes were closed (by the user or because their shell exited).
useSession.subscribe(({ view }) => {
  if (!view) return;
  const live = new Set(view.spaces.flatMap((s) => s.tabs.flatMap((t) => panesOf(t.layout))));
  for (const [pane, entry] of entries) {
    if (!live.has(pane)) {
      entry.webgl?.dispose();
      entry.term.dispose();
      entry.el.remove();
      entries.delete(pane);
    }
  }
});

// Settings apply live: theme, font and cursor changes reach every open terminal.
function applySettingsToTerminals() {
  fontsReady = fontReady();
  void fontsReady.then(() => {
    const options = terminalOptions();
    for (const [pane, entry] of entries) {
      Object.assign(entry.term.options, options);
      refit(pane, entry);
    }
  });
}
useSettings.subscribe(applySettingsToTerminals);
useSystemDark.subscribe(applySettingsToTerminals);
