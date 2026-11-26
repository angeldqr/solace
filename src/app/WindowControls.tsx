import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import type { ReactNode } from "react";
import { t } from "../shared/i18n";
import { IS_MAC } from "../shared/platform";

const appWindow = getCurrentWindow();

export function WindowControls() {
  if (IS_MAC) return null;
  return (
    <div className="flex h-full shrink-0">
      <WindowButton label={t("minimize")} onClick={() => appWindow.minimize()}>
        <Minus className="size-4" aria-hidden />
      </WindowButton>
      <WindowButton label={t("maximize")} onClick={() => appWindow.toggleMaximize()}>
        <Square className="size-3" aria-hidden />
      </WindowButton>
      <WindowButton label={t("close")} onClick={() => appWindow.close()} danger>
        <X className="size-4" aria-hidden />
      </WindowButton>
    </div>
  );
}

function WindowButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => unknown;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={() => void onClick()}
      className={`grid h-full w-[46px] place-items-center text-fg-muted transition-colors duration-100 ${
        danger ? "hover:bg-[#c42b1c] hover:text-white" : "hover:bg-hover hover:text-fg"
      }`}
    >
      {children}
    </button>
  );
}
