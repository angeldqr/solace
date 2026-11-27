import { useEffect, useState } from "react";
import { t } from "../../shared/i18n";
import { api, type Limit, type ProviderUsage, type Tokens, type UsageView } from "../../shared/ipc";
import { panesOf, useSession } from "../../shared/session";
import { AgentIcon } from "../../shared/ui/AgentIcon";
import { useNow } from "../../shared/useNow";
import { compact, duration, isStale, tone } from "./usage";

/** What the user spent; cache traffic is shown apart. */
const spent = (x: Tokens) => x.input + x.output;
const cached = (x: Tokens) => x.cacheCreation + x.cacheRead;
const total = (x: Tokens) => spent(x) + cached(x);
const add = (a: Tokens, b: Tokens): Tokens => ({
  input: a.input + b.input,
  output: a.output + b.output,
  cacheCreation: a.cacheCreation + b.cacheCreation,
  cacheRead: a.cacheRead + b.cacheRead,
});
const NONE: Tokens = { input: 0, output: 0, cacheCreation: 0, cacheRead: 0 };

function windowLabel(minutes: number) {
  if (minutes === 300) return t("usage5h");
  if (minutes === 10080) return t("usageWeekly");
  if (minutes === 43200) return t("usage30d");
  return `${minutes} min`;
}

function Bar({ percent }: { percent: number }) {
  const shown = Math.min(100, Math.max(0, percent));
  return (
    <div className="h-1.5 overflow-hidden rounded-full bg-hover">
      <div
        className={`h-full rounded-full bg-linear-to-r from-current/60 to-current transition-[width] duration-500 ${tone(percent)}`}
        style={{ width: `${shown}%` }}
      />
    </div>
  );
}

function LimitRow({ limit, now }: { limit: Limit; now: number }) {
  const reset = limit.resetsAt === null ? null : limit.resetsAt * 1000;
  // A passed reset time means the window started over: the reported percent is history.
  const stale = isStale(limit, now);
  return (
    <div className={`flex flex-col gap-1 ${stale ? "opacity-50" : ""}`}>
      <div className="flex items-baseline justify-between">
        <span className="text-fg-muted">{windowLabel(limit.windowMinutes)}</span>
        <span className="text-fg tabular-nums">
          {stale ? t("limitReset") : `${Math.round(limit.usedPercent)}%`}
        </span>
      </div>
      <Bar percent={stale ? 0 : limit.usedPercent} />
      {reset !== null && !stale && (
        <span
          className="text-2xs text-fg-subtle tabular-nums"
          title={new Date(reset).toLocaleString()}
        >
          {t("usageResetsIn", { time: duration(reset - now) })}
        </span>
      )}
    </div>
  );
}

function Provider(props: {
  agent: string;
  name: string;
  usage: ProviderUsage | null;
  now: number;
}) {
  if (!props.usage) return null;
  return (
    <section className="flex flex-col gap-3">
      <h3 className="flex items-center gap-2 font-medium text-fg">
        <AgentIcon agent={props.agent} className="size-4" />
        {props.name}
        {props.usage.plan && (
          <span className="rounded-full bg-hover px-1.5 font-normal text-2xs text-fg-muted">
            {props.usage.plan}
          </span>
        )}
      </h3>
      {props.usage.limits.map((l) => (
        <LimitRow key={l.windowMinutes} limit={l} now={props.now} />
      ))}
      <span className="text-2xs text-fg-subtle">
        {t("updatedAgo", { time: duration(props.now - props.usage.updatedAt) })}
      </span>
    </section>
  );
}

function Sparkline({ hourly, now }: { hourly: UsageView["tokens"]["hourly"]; now: number }) {
  const max = Math.max(1, ...hourly.map((h) => total(h.tokens)));
  const currentHour = Math.floor(now / 3600000) * 3600;
  return (
    <svg viewBox="0 0 96 24" className="h-6 w-full" preserveAspectRatio="none" aria-hidden>
      {hourly.map((h, i) => {
        const height = Math.max(1, (total(h.tokens) / max) * 22);
        return (
          <rect
            key={h.hour}
            x={i * 4 + 0.5}
            y={24 - height}
            width="3"
            height={height}
            rx="1"
            className="fill-primary"
            opacity={h.hour === currentHour ? 1 : 0.45}
          />
        );
      })}
    </svg>
  );
}

function Stat({ label, tokens }: { label: string; tokens: Tokens }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <span className="text-2xs text-fg-subtle">{label}</span>
      <span className="font-medium text-fg tabular-nums">{compact(spent(tokens))}</span>
      <span className="flex flex-wrap gap-x-2 text-2xs text-fg-subtle tabular-nums">
        <span>
          {t("usageIO", { input: compact(tokens.input), output: compact(tokens.output) })}
        </span>
        <span>{t("usageCache", { n: compact(cached(tokens)) })}</span>
      </span>
    </div>
  );
}

export function UsagePopover({
  view,
  onFocusPane,
}: {
  view: UsageView | null;
  onFocusPane?: () => void;
}) {
  const spaces = useSession((s) => s.view);
  const now = useNow(30_000);
  const [integration, setIntegration] = useState(true);
  useEffect(() => {
    void api.claudeIntegrationInstalled().then(setIntegration, () => {});
  }, []);

  // A provider not used for a week (an old Codex session, say) is noise, not usage.
  const recent = (u: ProviderUsage | null | undefined) =>
    u && now - u.updatedAt < 7 * 86_400_000 ? u : null;
  const claude = recent(view?.claude);
  const codex = recent(view?.codex);
  const empty = !claude && !codex;
  // shortcut: local midnight via the browser clock; a pre-midnight bucket straddling a half-hour offset is counted whole.
  const midnight = new Date(now).setHours(0, 0, 0, 0) / 1000;
  const today = (view?.tokens.hourly ?? [])
    .filter((h) => h.hour + 3600 > midnight)
    .reduce((sum, h) => add(sum, h.tokens), NONE);
  const hasTokens = total(view?.tokens.block ?? NONE) > 0 || total(today) > 0;

  return (
    <div className="flex flex-col gap-4 text-body">
      <Provider agent="claude" name="Claude Code" usage={claude} now={now} />
      <Provider agent="codex" name="Codex" usage={codex} now={now} />
      {empty && !hasTokens && (
        <div className="flex flex-col gap-1 text-fg-muted">
          <p>{t("usageEmpty")}</p>
          {!integration && <p className="text-2xs text-fg-subtle">{t("usageIntegrationOff")}</p>}
        </div>
      )}
      {empty && hasTokens && <p className="text-2xs text-fg-subtle">{t("usageNoLimits")}</p>}
      {view && hasTokens && (
        <section className="flex flex-col gap-2 border-line border-t pt-3">
          <div className="flex gap-4">
            <Stat label={t("usageBlock")} tokens={view.tokens.block} />
            <Stat label={t("usageToday")} tokens={today} />
          </div>
          <Sparkline hourly={view.tokens.hourly} now={now} />
        </section>
      )}
      {view && view.sessions.length > 0 && (
        <section className="flex flex-col gap-2 border-line border-t pt-3">
          <h3 className="text-2xs text-fg-subtle">{t("usageSessions")}</h3>
          {view.sessions.map((s) => {
            const space = spaces?.spaces.find((sp) =>
              sp.tabs.some((tab) => panesOf(tab.layout).includes(s.pane)),
            )?.name;
            return (
              <button
                key={s.pane}
                type="button"
                onClick={() => {
                  void api.paneFocus(s.pane);
                  onFocusPane?.();
                }}
                className="flex flex-col gap-1 rounded-md py-1 text-left hover:bg-hover"
              >
                <span className="flex items-center gap-2">
                  <AgentIcon agent={s.agent} className="size-4 shrink-0" />
                  <span className="min-w-0 flex-1 truncate text-fg">{s.model ?? s.agent}</span>
                  {space && <span className="truncate text-2xs text-fg-subtle">{space}</span>}
                  {s.costUsd !== null && (
                    <span className="text-fg-muted tabular-nums">${s.costUsd.toFixed(2)}</span>
                  )}
                </span>
                {s.percent !== null && s.usedTokens !== null && s.limit !== null && (
                  <span className="flex items-center gap-2" title={t("usageContext")}>
                    <span className="flex-1">
                      <Bar percent={s.percent} />
                    </span>
                    <span className="text-2xs text-fg-subtle tabular-nums">
                      {Math.round(s.percent)}% · {compact(s.usedTokens)} / {compact(s.limit)}
                    </span>
                  </span>
                )}
              </button>
            );
          })}
        </section>
      )}
    </div>
  );
}
