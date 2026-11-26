import type { AgentStatus } from "../agents";

/** Working spins, blocked pulses for attention, done and idle stay still. */
export function StatusDot({ status }: { status: AgentStatus }) {
  if (status === "working") {
    return (
      <span
        aria-hidden
        className="size-2.5 shrink-0 rounded-full border-[1.5px] border-primary-soft/30 border-t-primary-soft motion-safe:animate-spin"
      />
    );
  }
  const color = {
    blocked: "bg-warning motion-safe:animate-pulse",
    done: "bg-success",
    idle: "bg-fg-subtle/60",
  }[status];
  return <span aria-hidden className={`size-2 shrink-0 rounded-full ${color}`} />;
}
