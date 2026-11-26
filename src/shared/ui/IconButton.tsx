import type { ReactNode } from "react";
import { Tooltip } from "./Tooltip";

export function IconButton({
  label,
  onClick,
  children,
  className = "",
  shortcut,
  size = "md",
  pressed,
}: {
  label: string;
  onClick: () => unknown;
  children: ReactNode;
  className?: string;
  shortcut?: string;
  size?: "sm" | "md";
  pressed?: boolean;
}) {
  return (
    <Tooltip label={label} shortcut={shortcut}>
      <button
        type="button"
        aria-label={label}
        aria-pressed={pressed}
        onClick={() => void onClick()}
        className={`grid ${size === "sm" ? "size-5" : "size-6"} shrink-0 place-items-center rounded-md text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg [&>svg]:size-3.5 ${pressed ? "bg-hover text-fg" : ""} ${className}`}
      >
        {children}
      </button>
    </Tooltip>
  );
}
