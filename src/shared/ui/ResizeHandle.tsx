import { useState } from "react";

const STEP = 24;

/** Drag handle on one edge of a panel; the panel grows away from that edge. */
export function ResizeHandle({
  side,
  value,
  min,
  max,
  onChange,
  onReset,
  label,
}: {
  side: "left" | "right";
  value: number;
  min: number;
  /** A function is resolved on every interaction, so window resizes are honoured. */
  max: number | (() => number);
  onChange: (width: number) => void;
  onReset: () => void;
  label: string;
}) {
  const [dragging, setDragging] = useState(false);
  const limit = () => (typeof max === "function" ? max() : max);
  const clamp = (width: number) => Math.min(limit(), Math.max(min, width));
  /** What is on screen, which can differ from `value` (a panel temporarily widened). */
  const shown = (handle: HTMLElement) =>
    clamp(handle.parentElement?.getBoundingClientRect().width ?? value);
  const stopDrag = () => {
    setDragging(false);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  };

  return (
    // biome-ignore lint/a11y/useSemanticElements: an <hr> cannot be a drag handle
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={limit()}
      aria-valuenow={value}
      tabIndex={0}
      data-dragging={dragging || undefined}
      className={`absolute inset-y-0 z-10 w-2 cursor-col-resize outline-none after:absolute after:inset-y-0 after:left-1 after:w-px after:bg-transparent after:transition-colors hover:after:bg-primary focus-visible:after:bg-primary data-[dragging]:after:bg-primary ${
        side === "left" ? "-left-1" : "-right-1"
      }`}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        onChange(shown(event.currentTarget));
        setDragging(true);
        document.body.style.cursor = "col-resize";
        document.body.style.userSelect = "none";
      }}
      onPointerMove={(event) => {
        if (!dragging) return;
        const rect = event.currentTarget.parentElement?.getBoundingClientRect();
        if (!rect) return;
        onChange(clamp(side === "left" ? rect.right - event.clientX : event.clientX - rect.left));
      }}
      onPointerUp={stopDrag}
      onLostPointerCapture={stopDrag}
      onDoubleClick={onReset}
      onKeyDown={(event) => {
        const grow = side === "left" ? "ArrowLeft" : "ArrowRight";
        const shrink = side === "left" ? "ArrowRight" : "ArrowLeft";
        const delta = event.key === grow ? STEP : event.key === shrink ? -STEP : 0;
        if (!delta) return;
        event.preventDefault();
        onChange(clamp(shown(event.currentTarget) + delta));
      }}
    />
  );
}
