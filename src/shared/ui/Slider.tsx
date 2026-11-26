import { Slider as BaseSlider } from "@base-ui/react/slider";
import { useDescribedBy } from "./describedBy";

export function Slider({
  value,
  min,
  max,
  step = 1,
  onChange,
  label,
  format = String,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
  label: string;
  format?: (v: number) => string;
}) {
  const describedBy = useDescribedBy();
  return (
    <div className="flex w-56 items-center gap-3">
      <output className="min-w-12 rounded-md bg-hover px-2 py-0.5 text-center font-mono text-fg text-xs tabular-nums">
        {format(value)}
      </output>
      <BaseSlider.Root
        className="flex-1"
        value={value}
        min={min}
        max={max}
        step={step}
        onValueChange={(v) => onChange(Array.isArray(v) ? v[0] : v)}
      >
        <BaseSlider.Control className="flex h-5 w-full items-center">
          <BaseSlider.Track className="h-1.5 w-full rounded-full bg-line-strong">
            <BaseSlider.Indicator className="rounded-full bg-primary" />
            <BaseSlider.Thumb
              aria-label={label}
              aria-describedby={describedBy}
              className="size-4 rounded-full border-2 border-primary bg-canvas shadow transition-transform duration-100 hover:scale-110 data-dragging:scale-95"
            />
          </BaseSlider.Track>
        </BaseSlider.Control>
      </BaseSlider.Root>
    </div>
  );
}
