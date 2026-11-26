import { Popover } from "@base-ui/react/popover";
import { Gauge } from "lucide-react";
import { lazy, Suspense, useEffect } from "react";
import { t } from "../../shared/i18n";
import { Tooltip } from "../../shared/ui/Tooltip";
import { useNow } from "../../shared/useNow";
import { headline, startUsage, tone, useUsage } from "./usage";

// Keeps the popover's code out of the startup bundle.
const UsagePopover = lazy(() =>
  import("./UsagePopover").then((m) => ({ default: m.UsagePopover })),
);

const RADIUS = 6.5;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function Ring({ percent }: { percent: number }) {
  const shown = Math.min(100, Math.max(0, percent));
  return (
    <svg viewBox="0 0 16 16" className={`size-4 -rotate-90 ${tone(percent)}`} aria-hidden>
      <circle cx="8" cy="8" r={RADIUS} fill="none" strokeWidth="2" className="stroke-hover" />
      <circle
        cx="8"
        cy="8"
        r={RADIUS}
        fill="none"
        strokeWidth="2"
        strokeLinecap="round"
        stroke="currentColor"
        strokeDasharray={CIRCUMFERENCE}
        strokeDashoffset={CIRCUMFERENCE * (1 - shown / 100)}
        className="transition-[stroke-dashoffset] duration-500"
      />
    </svg>
  );
}

/** Top-bar button: the 5-hour usage at a glance, with the full meter in a popover. */
export function UsageButton() {
  const view = useUsage((s) => s.view);
  useEffect(startUsage, []);
  const now = useNow(30_000);
  const percent = headline(view, now);
  return (
    <Popover.Root>
      <Tooltip label={t("usage")}>
        <Popover.Trigger
          aria-label={percent === null ? t("usage") : `${t("usage")} ${Math.round(percent)}%`}
          className="flex h-8 items-center gap-1.5 rounded-md px-2.5 text-body text-fg-muted transition-colors duration-150 hover:bg-hover hover:text-fg data-popup-open:bg-selected data-popup-open:text-fg"
        >
          {percent === null ? (
            <>
              <Gauge className="size-4" aria-hidden />
              {t("usage")}
            </>
          ) : (
            <>
              <Ring percent={percent} />
              <span className="tabular-nums">{Math.round(percent)}%</span>
            </>
          )}
        </Popover.Trigger>
      </Tooltip>
      <Popover.Portal>
        <Popover.Positioner side="bottom" align="end" sideOffset={6} className="z-50">
          <Popover.Popup className="glass w-80 rounded-xl border border-line p-4 shadow-black/40 shadow-xl outline-none transition-[opacity,scale] duration-(--motion-duration) data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0">
            <Suspense fallback={null}>
              <UsagePopover view={view} />
            </Suspense>
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}
