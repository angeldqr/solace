import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@xterm/xterm/css/xterm.css";
import "./index.css";
import { error as logError } from "@tauri-apps/plugin-log";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { ErrorBoundary } from "./app/ErrorBoundary";
import { startFileEvents } from "./features/files/panel";
import { startSounds } from "./features/notify/sound";
import { startPresence } from "./features/presence/presence";
import { applyAppearance, useSettings, useSystemDark } from "./features/settings/settings";
import { startTasks } from "./features/tasks/tasks";
import { startUpdates } from "./features/updates/updates";
import { startAgents } from "./shared/agents";
import { locale } from "./shared/i18n";
import { api } from "./shared/ipc";
import { current, startSession } from "./shared/session";
import { TooltipProvider } from "./shared/ui/Tooltip";

document.documentElement.lang = locale;
// Notifications are sent by the backend; they should speak the UI's language.
void api.localeSet(locale);
applyAppearance();
useSettings.subscribe(() => applyAppearance());
useSystemDark.subscribe(() => applyAppearance());
// The browser's own menu (Back, Reload, Inspect...) means nothing in a desktop app.
document.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof Element && e.target.closest("input, textarea, [contenteditable]")))
    e.preventDefault();
});
// Failures outside React's render land in the log file too.
window.addEventListener("error", (e) => void logError(e.error?.stack ?? e.message).catch(() => {}));
window.addEventListener(
  "unhandledrejection",
  (e) => void logError(String(e.reason?.stack ?? e.reason)).catch(() => {}),
);
void startSession();
void startAgents();
void startTasks();
startUpdates();
startPresence();
startSounds();
void startFileEvents(() => current().space?.id);

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <TooltipProvider delay={400}>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </TooltipProvider>
  </StrictMode>,
);
