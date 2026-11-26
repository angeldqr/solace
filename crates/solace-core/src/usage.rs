//! Rate-limit and token usage, parsed from what Claude Code's status line and Codex's session logs
//! already record. Pure: callers do the IO.

use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limit {
    pub window_minutes: u32,
    pub used_percent: f64,
    /// Unix seconds.
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub limits: Vec<Limit>,
    pub plan: Option<String>,
    /// Unix milliseconds of the last report.
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSession {
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
    pub context_used_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
}

impl Add for Tokens {
    type Output = Tokens;
    fn add(self, o: Tokens) -> Tokens {
        Tokens {
            input: self.input + o.input,
            output: self.output + o.output,
            cache_creation: self.cache_creation + o.cache_creation,
            cache_read: self.cache_read + o.cache_read,
        }
    }
}

impl Tokens {
    /// Field-wise maximum.
    pub fn max(self, o: Tokens) -> Tokens {
        Tokens {
            input: self.input.max(o.input),
            output: self.output.max(o.output),
            cache_creation: self.cache_creation.max(o.cache_creation),
            cache_read: self.cache_read.max(o.cache_read),
        }
    }
}

impl AddAssign for Tokens {
    fn add_assign(&mut self, o: Tokens) {
        *self = *self + o;
    }
}

/// Unix seconds from a JSON number (integer or float) or a numeric string.
fn secs(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    let f = v
        .as_u64()
        .map(|n| n as f64)
        .or_else(|| v.as_f64())
        .or_else(|| v.as_str()?.trim().parse().ok())?;
    (f.is_finite() && f >= 0.0).then_some(f as u64)
}

fn claude_window(status: &Value, key: &str, minutes: u32) -> Option<Limit> {
    let w = status.get("rate_limits")?.get(key)?;
    Some(Limit {
        window_minutes: minutes,
        used_percent: w.get("used_percentage")?.as_f64()?,
        resets_at: secs(w.get("resets_at")),
    })
}

/// From the JSON Claude Code pipes to its status line. Limits are `None` when `rate_limits` is
/// absent (API-key users, or before the first response).
pub fn parse_claude_status(status: &Value) -> (Option<Vec<Limit>>, ClaudeSession) {
    let limits = status.get("rate_limits").map(|_| {
        [("five_hour", 300), ("seven_day", 10080)]
            .into_iter()
            .filter_map(|(key, minutes)| claude_window(status, key, minutes))
            .collect::<Vec<_>>()
    });
    let limits = limits.filter(|l| !l.is_empty());
    let ctx = status.get("context_window");
    let context_used_percent = ctx
        .and_then(|c| c.get("used_percentage"))
        .and_then(Value::as_f64);
    let session = ClaudeSession {
        model: status
            .pointer("/model/display_name")
            .and_then(Value::as_str)
            .map(str::to_string),
        cost_usd: status
            .pointer("/cost/total_cost_usd")
            .and_then(Value::as_f64),
        context_used_percent,
    };
    (limits, session)
}

/// A Codex `token_count` event with at least one rate-limit window. Windows are identified by
/// their length, never by position: free plans report a 30-day window as the primary one.
pub fn parse_codex_line(line: &str) -> Option<(Vec<Limit>, Option<String>)> {
    if !line.contains("token_count") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    let payload = v.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }
    let rl = payload.get("rate_limits")?;
    let stamp = v
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(iso_to_unix);
    let mut limits: Vec<Limit> = ["primary", "secondary"]
        .into_iter()
        .filter_map(|key| {
            let w = rl.get(key).filter(|w| w.is_object())?;
            let resets_at = secs(w.get("resets_at"))
                .or_else(|| Some(stamp? + secs(w.get("resets_in_seconds"))?));
            Some(Limit {
                window_minutes: w.get("window_minutes")?.as_u64()? as u32,
                used_percent: w.get("used_percent")?.as_f64()?,
                resets_at,
            })
        })
        .collect();
    if limits.is_empty() {
        return None;
    }
    limits.sort_by_key(|l| l.window_minutes);
    let plan = rl
        .get("plan_type")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some((limits, plan))
}

/// An assistant message from Claude Code's transcript: (dedupe key, unix seconds, tokens).
pub fn parse_claude_log_line(line: &str) -> Option<(String, u64, Tokens)> {
    if !line.contains("\"usage\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let msg = v.get("message")?;
    let usage = msg.get("usage")?;
    let n = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    let id = msg.get("id").and_then(Value::as_str).unwrap_or_default();
    let request = v
        .get("requestId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // Empty when the line has neither: callers must not merge those with each other.
    let key = if id.is_empty() && request.is_empty() {
        String::new()
    } else {
        format!("{id}:{request}")
    };
    let at = iso_to_unix(v.get("timestamp")?.as_str()?)?;
    let tokens = Tokens {
        input: n("input_tokens"),
        output: n("output_tokens"),
        cache_creation: n("cache_creation_input_tokens"),
        cache_read: n("cache_read_input_tokens"),
    };
    Some((key, at, tokens))
}

/// "YYYY-MM-DDTHH:MM:SS(.fff)Z" to unix seconds.
pub fn iso_to_unix(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + sec).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_to_unix("2026-11-26T20:49:46.869Z"), Some(1795726186));
        assert_eq!(iso_to_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_to_unix("2024-02-29T00:00:00Z"), Some(1709164800));
        assert_eq!(iso_to_unix("garbage"), None);
    }

    #[test]
    fn claude_status_with_and_without_limits() {
        let (limits, s) = parse_claude_status(&json!({
            "model": {"display_name": "Opus"},
            "cost": {"total_cost_usd": 0.42},
            "context_window": {"used_percentage": 12.5},
            "rate_limits": {
                "five_hour": {"used_percentage": 23.5, "resets_at": 1000},
                "seven_day": {"used_percentage": 41.2}
            }
        }));
        let limits = limits.unwrap();
        assert_eq!(limits.len(), 2);
        assert_eq!(limits[0].window_minutes, 300);
        assert_eq!(limits[0].resets_at, Some(1000));
        assert_eq!(limits[1].window_minutes, 10080);
        assert_eq!(limits[1].resets_at, None);
        assert_eq!(s.model.as_deref(), Some("Opus"));
        assert_eq!(s.cost_usd, Some(0.42));
        assert_eq!(s.context_used_percent, Some(12.5));

        let (limits, s) = parse_claude_status(&json!({"model": {"display_name": "Sonnet"}}));
        assert!(limits.is_none());
        assert_eq!(s.model.as_deref(), Some("Sonnet"));
        assert_eq!(s.cost_usd, None);

        let (limits, _) =
            parse_claude_status(&json!({"rate_limits": {"five_hour": {"used_percentage": 5.0}}}));
        assert_eq!(limits.unwrap().len(), 1);
    }

    #[test]
    fn codex_lines() {
        let line = r#"{"timestamp":"2026-11-26T20:49:46.869Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":12.5,"window_minutes":300,"resets_at":2000.0},"secondary":{"used_percent":3.0,"window_minutes":10080,"resets_in_seconds":"60"},"plan_type":"plus"}}}"#;
        let (limits, plan) = parse_codex_line(line).unwrap();
        assert_eq!(plan.as_deref(), Some("plus"));
        assert_eq!(limits[0].resets_at, Some(2000));
        assert_eq!(limits[1].resets_at, Some(1795726186 + 60));

        let free = r#"{"timestamp":"2026-11-26T20:49:46Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":null,"secondary":{"used_percent":9.0,"window_minutes":43200}}}}"#;
        let (limits, _) = parse_codex_line(free).unwrap();
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].window_minutes, 43200);

        let empty = r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":null,"secondary":null}}}"#;
        assert!(parse_codex_line(empty).is_none());
        assert!(
            parse_codex_line(r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#)
                .is_none()
        );
        assert!(parse_codex_line("not json token_count").is_none());
    }

    #[test]
    fn claude_log_lines() {
        let line = r#"{"type":"assistant","requestId":"req1","timestamp":"2026-11-26T20:49:46.869Z","message":{"id":"msg1","usage":{"input_tokens":3,"output_tokens":4,"cache_creation_input_tokens":5,"cache_read_input_tokens":6}}}"#;
        let (key, at, t) = parse_claude_log_line(line).unwrap();
        assert_eq!(key, "msg1:req1");
        assert_eq!(at, 1795726186);
        assert_eq!(
            t,
            Tokens {
                input: 3,
                output: 4,
                cache_creation: 5,
                cache_read: 6
            }
        );
        assert!(parse_claude_log_line(r#"{"type":"user","message":{"usage":{}}}"#).is_none());
        assert!(
            parse_claude_log_line(r#"{"type":"assistant","message":{"content":"x"}}"#).is_none()
        );
        assert!(parse_claude_log_line("{").is_none());
    }
}
