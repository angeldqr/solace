import { Switch as BaseSwitch } from "@base-ui/react/switch";

export function Switch({
  checked,
  onCheckedChange,
  label,
  disabled,
}: {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <BaseSwitch.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={label}
      className="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full bg-line-strong p-0.5 transition-colors duration-200 data-checked:bg-primary data-disabled:cursor-not-allowed data-disabled:opacity-50"
    >
      <BaseSwitch.Thumb className="size-4 rounded-full bg-white shadow-sm ring-1 ring-black/10 transition-transform duration-150 data-checked:translate-x-4" />
    </BaseSwitch.Root>
  );
}
