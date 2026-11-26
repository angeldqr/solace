import { Tooltip as BaseTooltip } from "@base-ui/react/tooltip";
import type { ReactElement } from "react";
import { Kbd } from "./Kbd";

export const TooltipProvider = BaseTooltip.Provider;

export function Tooltip({
  label,
  shortcut,
  children,
}: {
  label: string;
  shortcut?: string;
  children: ReactElement;
}) {
  return (
    <BaseTooltip.Root>
      <BaseTooltip.Trigger render={children} />
      <BaseTooltip.Portal>
        <BaseTooltip.Positioner sideOffset={6} className="z-[60]">
          <BaseTooltip.Popup className="flex items-center gap-2 rounded-md border border-line bg-surface px-2 py-1 text-fg text-xs shadow-black/40 shadow-lg transition-opacity duration-100 data-ending-style:opacity-0 data-starting-style:opacity-0">
            {label}
            {shortcut && <Kbd keys={shortcut} />}
          </BaseTooltip.Popup>
        </BaseTooltip.Positioner>
      </BaseTooltip.Portal>
    </BaseTooltip.Root>
  );
}
