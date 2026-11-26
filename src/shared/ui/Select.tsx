import { Select as BaseSelect } from "@base-ui/react/select";
import { Check, ChevronsUpDown } from "lucide-react";

export function Select<T extends string>({
  value,
  options,
  onChange,
  label,
  className = "",
}: {
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (v: T) => void;
  label: string;
  className?: string;
}) {
  return (
    <BaseSelect.Root value={value} items={options} onValueChange={(v) => v !== null && onChange(v)}>
      <BaseSelect.Trigger
        aria-label={label}
        className={`flex h-8 min-w-40 items-center justify-between gap-2 rounded-lg border border-line bg-canvas/40 px-2.5 text-body text-fg transition-colors duration-150 hover:bg-hover disabled:pointer-events-none disabled:opacity-50 ${className}`}
      >
        <BaseSelect.Value />
        <BaseSelect.Icon>
          <ChevronsUpDown className="size-3.5 text-fg-subtle" />
        </BaseSelect.Icon>
      </BaseSelect.Trigger>
      <BaseSelect.Portal>
        <BaseSelect.Positioner sideOffset={4} alignItemWithTrigger={false} className="z-50">
          <BaseSelect.Popup className="glass min-w-(--anchor-width) rounded-lg border border-line p-1 shadow-black/40 shadow-xl outline-none transition-[opacity,scale] duration-150 data-ending-style:opacity-0 data-starting-style:scale-[0.98] data-starting-style:opacity-0">
            {options.map((o) => (
              <BaseSelect.Item
                key={o.value}
                value={o.value}
                className="flex h-8 cursor-default select-none items-center gap-2 rounded-md px-2 text-body text-fg-muted outline-none transition-colors duration-150 data-highlighted:bg-hover data-highlighted:text-fg data-selected:text-fg"
              >
                <span className="w-3.5">
                  <BaseSelect.ItemIndicator>
                    <Check className="size-3.5" />
                  </BaseSelect.ItemIndicator>
                </span>
                <BaseSelect.ItemText>{o.label}</BaseSelect.ItemText>
              </BaseSelect.Item>
            ))}
          </BaseSelect.Popup>
        </BaseSelect.Positioner>
      </BaseSelect.Portal>
    </BaseSelect.Root>
  );
}
