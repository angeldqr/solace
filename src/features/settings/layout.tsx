import type { ReactNode } from "react";

export function Section({
  title,
  children,
  action,
}: {
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-4">
        <h3 className="font-semibold text-base text-fg">{title}</h3>
        {action}
      </div>
      {children}
    </section>
  );
}

export function Card({ children }: { children: ReactNode }) {
  return (
    <div className="divide-y divide-line rounded-xl border border-line bg-surface/40">
      {children}
    </div>
  );
}

export function Row({
  title,
  description,
  children,
  htmlFor,
}: {
  title: ReactNode;
  description?: string;
  children: ReactNode;
  htmlFor?: string;
}) {
  const titleClass = "font-medium text-body text-fg";
  return (
    <div className="flex items-center justify-between gap-8 px-4 py-3.5">
      <div className="min-w-0">
        {htmlFor ? (
          <label htmlFor={htmlFor} className={titleClass}>
            {title}
          </label>
        ) : (
          <div className={titleClass}>{title}</div>
        )}
        {description && (
          <p className="mt-0.5 max-w-prose text-fg-muted text-xs leading-relaxed">{description}</p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

export function Hint({
  children,
  tone = "muted",
  id,
}: {
  children: ReactNode;
  tone?: "muted" | "danger";
  id?: string;
}) {
  return (
    <p id={id} className={`text-xs ${tone === "danger" ? "text-danger" : "text-fg-muted"}`}>
      {children}
    </p>
  );
}
