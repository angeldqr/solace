/** Renders a combo like "Ctrl+Shift+P" as keycaps. */
export function Kbd({ keys, className }: { keys: string; className?: string }) {
  return (
    <span className="inline-flex items-center gap-0.5">
      {keys.split("+").map((key, i) => (
        <kbd
          // biome-ignore lint/suspicious/noArrayIndexKey: keycaps have no identity beyond position
          key={i}
          className={`inline-flex h-5 min-w-5 items-center justify-center rounded-md border px-1 font-sans text-2xs ${
            className ?? "border-line bg-hover text-fg-muted"
          }`}
        >
          {key}
        </kbd>
      ))}
    </span>
  );
}
