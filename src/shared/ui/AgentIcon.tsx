import { Bot } from "lucide-react";
import { agentIcons } from "../icons/agentIcons";

export function AgentIcon({
  agent,
  className = "size-3.5",
}: {
  agent: string;
  className?: string;
}) {
  const icon = agentIcons[agent];
  if (!icon) return <Bot className={`${className} text-fg-muted`} aria-hidden />;
  return (
    <img src={icon.src} alt="" className={`${className} shrink-0 ${icon.mono ? "invert" : ""}`} />
  );
}
