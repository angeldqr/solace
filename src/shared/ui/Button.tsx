import type { ButtonHTMLAttributes } from "react";

const VARIANT = {
  primary: "bg-primary text-on-primary hover:brightness-110",
  outline: "border border-line bg-canvas/40 text-fg hover:bg-hover",
  ghost: "text-fg-muted hover:bg-hover hover:text-fg",
};
const SIZE = { sm: "h-7 px-2.5 text-xs", md: "h-8 px-3 text-body" };

export function Button({
  variant = "outline",
  size = "md",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: keyof typeof VARIANT;
  size?: keyof typeof SIZE;
}) {
  return (
    <button
      type="button"
      className={`inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-lg font-medium transition-[background-color,color,scale] duration-150 active:scale-[0.98] disabled:pointer-events-none disabled:opacity-50 [&>svg]:size-4 ${VARIANT[variant]} ${SIZE[size]} ${className}`}
      {...props}
    />
  );
}
