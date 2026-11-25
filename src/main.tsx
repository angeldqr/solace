import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@xterm/xterm/css/xterm.css";
import "./index.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { startFileEvents } from "./features/files/panel";
import { activeTheme, applyTheme, useSettings } from "./features/settings/settings";
import { startUpdates } from "./features/updates/updates";
import { startAgents } from "./shared/agents";
import { locale } from "./shared/i18n";
import { api } from "./shared/ipc";
import { current, startSession } from "./shared/session";

document.documentElement.lang = locale;
// Notifications are sent by the backend; they should speak the UI's language.
void api.localeSet(locale);
applyTheme(activeTheme());
useSettings.subscribe((settings) => applyTheme(activeTheme(settings)));
void startSession();
void startAgents();
startUpdates();
void startFileEvents(() => current().space?.id);

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
