import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@xterm/xterm/css/xterm.css";
import "./index.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { startFileEvents } from "./features/files/panel";
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
void startSession();
void startAgents();
void startTasks();
startUpdates();
startPresence();
void startFileEvents(() => current().space?.id);

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <TooltipProvider delay={400}>
      <App />
    </TooltipProvider>
  </StrictMode>,
);
