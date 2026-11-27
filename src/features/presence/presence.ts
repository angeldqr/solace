import { useAgents } from "../../shared/agents";
import { api } from "../../shared/ipc";
import { current, useSession } from "../../shared/session";
import { useSettings } from "../settings/settings";
import { activityFor } from "./activity";

export function startPresence() {
  let sent = "";
  const sync = () => {
    const next = useSettings.getState().discordActivity
      ? activityFor(useAgents.getState().list, current().pane)
      : null;
    const key = JSON.stringify(next);
    if (key === sent) return;
    sent = key;
    void api.presenceUpdate(next).catch(() => {});
  };
  useSettings.subscribe(sync);
  useAgents.subscribe(sync);
  useSession.subscribe(sync);
  window.addEventListener("beforeunload", () => void api.presenceUpdate(null).catch(() => {}));
  sync();
}
