//! Reads usage from the files Codex and Claude Code already keep on disk.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use solace_core::usage::{
    ContextUse, ProviderUsage, Tokens, parse_claude_log_line, parse_codex_context,
    parse_codex_line, parse_codex_meta,
};

const TAIL: u64 = 1024 * 1024;
const DAY: u64 = 24 * 3600;

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Child directories (or files) of `dir`, names sorted newest (largest) first.
fn sorted_desc(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    v.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    v
}

/// Rollouts of the two newest day directories, newest modification first.
fn rollouts(sessions: &Path) -> Vec<(PathBuf, SystemTime)> {
    let mut days = 0;
    let mut found = Vec::new();
    'walk: for year in sorted_desc(sessions) {
        for month in sorted_desc(&year) {
            for day in sorted_desc(&month) {
                for f in sorted_desc(&day) {
                    let name = f.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                    if !(name.starts_with("rollout-") && name.ends_with(".jsonl")) {
                        continue;
                    }
                    if let Ok(m) = fs::metadata(&f).and_then(|m| m.modified()) {
                        found.push((f, m));
                    }
                }
                days += 1;
                if days == 2 {
                    break 'walk;
                }
            }
        }
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.1));
    found
}

fn read_tail(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(TAIL))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Remembers the last result per newest file, so an unchanged rollout isn't read again.
#[derive(Default)]
pub struct CodexCache {
    key: Option<(PathBuf, SystemTime)>,
    value: Option<ProviderUsage>,
}

/// How many recent rollouts to look through for one that has reported limits.
const FALLBACK_FILES: usize = 8;

impl CodexCache {
    pub fn read(&mut self, home: &Path) -> Option<ProviderUsage> {
        let files = rollouts(&home.join("sessions"));
        let newest = files.first()?.clone();
        if self.key.as_ref() == Some(&newest) {
            return self.value.clone();
        }
        // A fresh rollout has no token_count until the first reply: fall back to the one before.
        let value = files.iter().take(FALLBACK_FILES).find_map(|(path, mtime)| {
            // The first line of the window may be cut mid-way; it just fails to parse.
            let text = read_tail(path)?;
            let (limits, plan) = text.lines().rev().find_map(parse_codex_line)?;
            Some(ProviderUsage {
                limits,
                plan,
                updated_at: unix_secs(*mtime) * 1000,
            })
        });
        self.key = Some(newest);
        self.value = value.clone();
        value
    }
}

/// The latest Codex rate limits under `home` (`~/.codex`).
pub fn codex_limits(home: &Path) -> Option<ProviderUsage> {
    CodexCache::default().read(home)
}

/// Same folder, whatever the separators, case or trailing slash.
fn same_dir(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm(a) == norm(b)
}

/// Context held by Codex sessions, read from the tail of their rollouts.
#[derive(Default)]
pub struct CodexContext {
    /// Per rollout: (mtime, session cwd, context at that mtime), so unchanged files aren't re-read.
    /// The cwd never changes, so a modified rollout only has its tail read again.
    files: HashMap<PathBuf, (SystemTime, Option<String>, Option<ContextUse>)>,
}

impl CodexContext {
    /// The newest recent rollout started in `cwd`. Two Codex panes in one folder share the answer.
    pub fn read(&mut self, home: &Path, cwd: &Path) -> Option<ContextUse> {
        let want = cwd.to_string_lossy();
        let files = rollouts(&home.join("sessions"));
        self.files.retain(|p, _| files.iter().any(|(f, _)| f == p));
        for (path, mtime) in files {
            let old = self.files.get(&path);
            let (_, session_cwd, ctx) = match old {
                Some(k) if k.0 == mtime => k.clone(),
                _ => {
                    let cwd = match old {
                        Some(k) => k.1.clone(),
                        None => first_cwd(&path),
                    };
                    let k = (mtime, cwd, tail_context(&path));
                    self.files.insert(path, k.clone());
                    k
                }
            };
            if session_cwd.is_some_and(|c| same_dir(&c, &want)) {
                return ctx;
            }
        }
        None
    }
}

fn first_cwd(path: &Path) -> Option<String> {
    let mut line = String::new();
    BufReader::new(fs::File::open(path).ok()?)
        .read_line(&mut line)
        .ok()?;
    parse_codex_meta(&line)?.0
}

fn tail_context(path: &Path) -> Option<ContextUse> {
    let text = read_tail(path)?;
    let mut ctx = text.lines().rev().find_map(parse_codex_context)?;
    ctx.model = text.lines().rev().find_map(|l| parse_codex_meta(l)?.1);
    Some(ctx)
}

struct FileState {
    /// Bytes consumed so far, always at a line boundary.
    offset: u64,
    len: u64,
    mtime: SystemTime,
}

/// Token usage of the last 24 hours, read incrementally from Claude Code's transcripts.
#[derive(Default)]
pub struct ClaudeLog {
    files: HashMap<PathBuf, FileState>,
    seen: HashMap<String, (u64, Tokens)>,
}

/// True when the file was rewritten rather than appended to: it shrank below what we consumed, or
/// the byte before our offset is no longer a line end.
fn rewritten(f: &mut fs::File, state: &FileState, len: u64, mtime: SystemTime) -> bool {
    if mtime == state.mtime || state.offset == 0 {
        return false;
    }
    let mut byte = [0u8];
    len < state.offset
        || f.seek(SeekFrom::Start(state.offset - 1)).is_err()
        || f.read_exact(&mut byte).is_err()
        || byte[0] != b'\n'
}

impl ClaudeLog {
    pub fn scan(&mut self, projects: &Path, now: u64) {
        let cutoff = now.saturating_sub(DAY);
        let mut paths = Vec::new();
        collect(projects, 0, cutoff, &mut paths);
        self.files.retain(|p, _| paths.contains(p));
        for path in paths {
            let Ok(mut f) = fs::File::open(&path) else {
                continue;
            };
            let Ok(meta) = f.metadata() else { continue };
            let (len, mtime) = (meta.len(), meta.modified().unwrap_or(UNIX_EPOCH));
            let state = self.files.entry(path.clone()).or_insert(FileState {
                offset: 0,
                len: 0,
                mtime: UNIX_EPOCH,
            });
            if rewritten(&mut f, state, len, mtime) {
                state.offset = 0;
            }
            state.len = len;
            state.mtime = mtime;
            if len <= state.offset || f.seek(SeekFrom::Start(state.offset)).is_err() {
                continue;
            }
            // Line by line: memory stays bounded by the longest line, and a partial last line
            // (the writer is mid-write) is left for the next scan.
            let mut reader = BufReader::new(f);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(n) if n > 0 && buf.ends_with(b"\n") => {
                        if let Some((key, at, tokens)) =
                            parse_claude_log_line(&String::from_utf8_lossy(&buf))
                            && at >= cutoff
                        {
                            // Streaming rewrites a message with growing counts: keep the largest.
                            let key = if key.is_empty() {
                                format!("{}@{}", path.display(), state.offset)
                            } else {
                                key
                            };
                            let entry = self.seen.entry(key).or_insert((at, Tokens::default()));
                            entry.1 = entry.1.max(tokens);
                        }
                        state.offset += n as u64;
                    }
                    _ => break,
                }
            }
        }
        self.seen.retain(|_, (at, _)| *at >= cutoff);
    }

    pub fn totals_since(&self, since: u64) -> Tokens {
        self.seen
            .values()
            .filter(|(at, _)| *at >= since)
            .fold(Tokens::default(), |acc, (_, t)| acc + *t)
    }

    /// 24 hourly buckets, oldest first, the last one containing `now`.
    pub fn hourly(&self, now: u64) -> Vec<(u64, Tokens)> {
        let last = now / 3600 * 3600;
        (0..24)
            .map(|i| {
                let hour = last - (23 - i) * 3600;
                let sum = self
                    .seen
                    .values()
                    .filter(|(at, _)| (hour..hour + 3600).contains(at))
                    .fold(Tokens::default(), |acc, (_, t)| acc + *t);
                (hour, sum)
            })
            .collect()
    }
}

/// `.jsonl` files modified since `cutoff`, down to `projects/<dir>/<session>/subagents/`.
fn collect(dir: &Path, depth: u8, cutoff: u64, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            if depth < 3 {
                collect(&path, depth + 1, cutoff, out);
            }
        } else if path.extension().is_some_and(|x| x == "jsonl")
            && meta.modified().is_ok_and(|m| unix_secs(m) >= cutoff)
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("solace-usage-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn line(id: &str, ts: &str, out: u64) -> String {
        format!(
            r#"{{"type":"assistant","requestId":"r","timestamp":"{ts}","message":{{"id":"{id}","usage":{{"input_tokens":1,"output_tokens":{out}}}}}}}"#
        )
    }

    // 2026-11-26T20:49:46Z
    const NOW: u64 = 1795726186;
    const TS: &str = "2026-11-26T20:49:46.000Z";

    #[test]
    fn partial_lines_wait_and_duplicates_count_once() {
        let d = temp("claude");
        let a = d.join("p1/a.jsonl");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        let full = line("m1", TS, 10);
        fs::write(&a, format!("{full}\n{}", &line("m2", TS, 5)[..30])).unwrap();
        let mut log = ClaudeLog::default();
        log.scan(&d, NOW);
        assert_eq!(log.totals_since(0).output, 10);
        // The writer finishes the line.
        let mut f = fs::OpenOptions::new().append(true).open(&a).unwrap();
        writeln!(f, "{}", &line("m2", TS, 5)[30..]).unwrap();
        log.scan(&d, NOW);
        assert_eq!(log.totals_since(0).output, 15);
        // The same message resumed in another file.
        let b = d.join("p2/b.jsonl");
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fs::write(&b, format!("{full}\n")).unwrap();
        log.scan(&d, NOW);
        assert_eq!(log.totals_since(0).output, 15);
        let hourly = log.hourly(NOW);
        assert_eq!(hourly.len(), 24);
        assert_eq!(hourly[23].1.output, 15);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn old_files_and_entries_are_skipped() {
        let d = temp("old");
        let a = d.join("p/a.jsonl");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        fs::write(&a, format!("{}\n", line("m1", TS, 10))).unwrap();
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(NOW - 3 * DAY);
        fs::File::options()
            .write(true)
            .open(&a)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let mut log = ClaudeLog::default();
        log.scan(&d, NOW);
        assert_eq!(log.totals_since(0).output, 0);
        // A fresh file holding an entry older than a day is dropped too.
        let b = d.join("p/b.jsonl");
        fs::write(&b, format!("{}\n", line("m2", "2026-11-20T00:00:00Z", 7))).unwrap();
        log.scan(&d, NOW + 10);
        assert_eq!(log.totals_since(0).output, 0);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn codex_picks_the_newest_rollout_by_mtime() {
        let d = temp("codex");
        let day = d.join("sessions/2026/11/26");
        fs::create_dir_all(&day).unwrap();
        let rl = |pct: f64| {
            format!(
                r#"{{"timestamp":"{TS}","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":{pct},"window_minutes":300,"resets_at":5}},"secondary":null,"plan_type":"plus"}}}}}}"#
            )
        };
        let old = day.join("rollout-a.jsonl");
        let new = day.join("rollout-b.jsonl");
        fs::write(&old, format!("{}\n", rl(11.0))).unwrap();
        fs::write(&new, format!("{}\n{{\"type\":\"other\"}}\n", rl(42.0))).unwrap();
        let t = SystemTime::now() - Duration::from_secs(100);
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(t)
            .unwrap();
        let usage = codex_limits(&d).unwrap();
        assert_eq!(usage.limits[0].used_percent, 42.0);
        assert_eq!(usage.plan.as_deref(), Some("plus"));
        // Cached while unchanged.
        let mut cache = CodexCache::default();
        assert!(cache.read(&d).is_some());
        assert!(cache.read(&d).is_some());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn max_per_key_anonymous_lines_and_rewrites() {
        let d = temp("misc");
        let a = d.join("p/a.jsonl");
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        let anon = format!(
            r#"{{"type":"assistant","timestamp":"{TS}","message":{{"usage":{{"input_tokens":2,"output_tokens":3}}}}}}"#
        );
        // The same message streamed twice with growing output, plus two id-less lines.
        fs::write(
            &a,
            format!(
                "{}\n{}\n{anon}\n{anon}\n",
                line("m1", TS, 4),
                line("m1", TS, 9)
            ),
        )
        .unwrap();
        let mut log = ClaudeLog::default();
        log.scan(&d, NOW);
        assert_eq!(log.totals_since(0).output, 9 + 3 + 3);

        // Rewritten with other content of at least the same length: read again from the start.
        fs::write(&a, format!("{}\n", line("m9", TS, 50))).unwrap();
        let later = SystemTime::now() + Duration::from_secs(5);
        fs::File::options()
            .write(true)
            .open(&a)
            .unwrap()
            .set_modified(later)
            .unwrap();
        log.scan(&d, NOW);
        assert_eq!(log.totals_since(0).output, 9 + 3 + 3 + 50);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn codex_context_by_cwd() {
        let d = temp("codex-ctx");
        let day = d.join("sessions/2026/11/26");
        fs::create_dir_all(&day).unwrap();
        let tc = |n: u64| {
            format!(
                r#"{{"type":"event_msg","payload":{{"type":"token_count","info":{{"last_token_usage":{{"input_tokens":{n}}},"model_context_window":1000}}}}}}"#
            )
        };
        let meta = |cwd: &str| format!(r#"{{"type":"session_meta","payload":{{"cwd":"{cwd}"}}}}"#);
        let turn = r#"{"type":"turn_context","payload":{"model":"gpt-5"}}"#;
        fs::write(
            day.join("rollout-a.jsonl"),
            format!(
                "{}\n{turn}\n{}\n{}\n",
                meta("C:\\\\Proj\\\\A"),
                tc(100),
                tc(300)
            ),
        )
        .unwrap();
        fs::write(
            day.join("rollout-b.jsonl"),
            format!("{}\n{}\n", meta("C:\\\\Other"), tc(5)),
        )
        .unwrap();
        let mut cache = CodexContext::default();
        let c = cache.read(&d, Path::new("c:/proj/a/")).unwrap();
        assert_eq!(
            (c.used, c.limit, c.model.as_deref()),
            (300, Some(1000), Some("gpt-5"))
        );
        assert!(cache.read(&d, Path::new("C:/nowhere")).is_none());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn codex_falls_back_when_the_newest_rollout_has_no_limits() {
        let d = temp("codex-fallback");
        let day = d.join("sessions/2026/11/26");
        fs::create_dir_all(&day).unwrap();
        let old = day.join("rollout-a.jsonl");
        fs::write(&old, format!(
            r#"{{"timestamp":"{TS}","type":"event_msg","payload":{{"type":"token_count","rate_limits":{{"primary":{{"used_percent":7.0,"window_minutes":300}}}}}}}}"#
        ) + "\n").unwrap();
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(100))
            .unwrap();
        fs::write(day.join("rollout-b.jsonl"), "{\"type\":\"session_meta\"}\n").unwrap();
        assert_eq!(codex_limits(&d).unwrap().limits[0].used_percent, 7.0);
        fs::remove_dir_all(&d).unwrap();
    }
}
