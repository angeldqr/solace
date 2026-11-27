import { error as logError } from "@tauri-apps/plugin-log";
import { Component, type ReactNode } from "react";
import { t } from "../shared/i18n";
import { Button } from "../shared/ui/Button";

/** Last resort: a render error shows this instead of a blank window. The backend keeps running. */
export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error) {
    void logError(error.stack ?? error.message).catch(() => {});
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-4 bg-canvas p-8 text-center text-fg">
        <img src="/solace-mark.svg" alt="" className="size-12" />
        <h1 className="font-medium text-lg">{t("uiCrashedTitle")}</h1>
        <p className="text-fg-muted text-sm">{t("uiCrashedBody")}</p>
        <div className="flex gap-2">
          <Button variant="primary" onClick={() => location.reload()}>
            {t("reload")}
          </Button>
          <Button
            onClick={() =>
              void navigator.clipboard.writeText(error.stack ?? error.message).catch(() => {})
            }
          >
            {t("copyDetails")}
          </Button>
        </div>
      </div>
    );
  }
}
