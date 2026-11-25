import type { ReactNode } from "react";

export function IconButton({
  label,
  onClick,
  children,
  className = "",
}: {
  label: string;
  onClick: () => unknown;
  children: ReactNode;
  className?: string;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={() => void onClick()}
      className={`grid size-6 shrink-0 place-items-center rounded-md text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg [&>svg]:size-3.5 ${className}`}
    >
      {children}
    </button>
  );
}
