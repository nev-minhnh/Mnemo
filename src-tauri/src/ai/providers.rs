//! Talking to the AI providers: the Claude Code and Codex CLIs (the person's own signed-in
//! accounts), OpenAI-compatible APIs (OpenAI, Gemini, xAI, DeepSeek, MiniMax, OpenRouter,
//! llama.cpp…) and the Anthropic API.

use crate::settings::{AiChoice, ProviderConfig, ProviderKind};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Request<'a> {
    pub system: &'a str,
    pub user: &'a str,
    pub choice: &'a AiChoice,
}

const CLI_TIMEOUT: Duration = Duration::from_secs(240);
const HTTP_TIMEOUT: Duration = Duration::from_secs(240);

fn effort_or(choice: &AiChoice, fallback: &str) -> String {
    let e = choice.effort.trim();
    if e.is_empty() {
        fallback.to_string()
    } else {
        e.to_string()
    }
}

/// Finds a CLI on PATH (and the places installers put it), preferring what Windows can start.
pub fn find_program(name: &str, configured: &str) -> Option<PathBuf> {
    if !configured.trim().is_empty() {
        let p = PathBuf::from(configured.trim());
        return p.is_file().then_some(p);
    }
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
        dirs.push(home.join("AppData").join("Roaming").join("npm"));
        dirs.push(home.join(".bun").join("bin"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    for d in dirs {
        for e in exts {
            let p = d.join(format!("{name}{e}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Runs a program with `input` on stdin (never on the command line), a time limit,
/// no console window, and output read on threads so pipes never fill.
fn run_cli(
    program: &Path,
    args: &[String],
    input: &str,
    envs: &[(&str, &str)],
    cwd: &Path,
) -> Result<(i32, String, String), String> {
    let is_script = program
        .extension()
        .map(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        .unwrap_or(false);
    let mut cmd = if is_script {
        // npm shims: run through cmd.exe. Every argument here is a plain word.
        let mut c = Command::new("cmd.exe");
        c.arg("/D").arg("/C").arg(program);
        c
    } else {
        Command::new(program)
    };
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Không chạy được {}: {e}", program.display()))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let input = input.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut out = child.stdout.take().ok_or("no stdout")?;
    let err = child.stderr.take().ok_or("no stderr")?;
    let out_t = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let err_t = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.take(64 * 1024).read_to_end(&mut s);
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > CLI_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{} không trả lời sau {} giây",
                    program.display(),
                    CLI_TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = writer.join();
    let stdout = String::from_utf8_lossy(&out_t.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).into_owned();
    Ok((status.code().unwrap_or(-1), stdout, stderr))
}

fn claude_code(p: &ProviderConfig, req: &Request, work: &Path) -> Result<String, String> {
    let program = find_program("claude", &p.cli_path)
        .ok_or("Không tìm thấy Claude Code (claude). Hãy cài Claude Code và đăng nhập, hoặc chỉ đường dẫn trong Cài đặt → AI.")?;
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "json",
        "--no-session-persistence",
        "--disallowedTools",
        "*",
        "--strict-mcp-config",
        "--disable-slash-commands",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let model = req.choice.model.trim();
    if !model.is_empty() && model != "default" {
        args.push("--model".into());
        args.push(model.into());
    }
    let effort = req.choice.effort.trim();
    if !effort.is_empty() {
        args.push("--effort".into());
        args.push(effort.into());
    }
    let mut envs = vec![];
    if !req.choice.thinking {
        envs.push(("MAX_THINKING_TOKENS", "0"));
    }
    // The system prompt travels with the input, so nothing long goes on the command line.
    let input = format!(
        "<instructions>\n{}\n</instructions>\n\n{}",
        req.system, req.user
    );
    let (code, stdout, stderr) = run_cli(&program, &args, &input, &envs, work)?;
    if let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) {
        let text = v
            .get("result")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        if v.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false) {
            return Err(format!(
                "Claude Code: {}",
                if text.is_empty() {
                    stderr.trim()
                } else {
                    &text
                }
            ));
        }
        return Ok(text);
    }
    if code != 0 {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(format!(
            "Claude Code dừng với mã {code}: {}",
            crate::util::truncate_chars(detail, 600)
        ));
    }
    Ok(stdout.trim().to_string())
}

fn codex(p: &ProviderConfig, req: &Request, work: &Path) -> Result<String, String> {
    let program = find_program("codex", &p.cli_path)
        .ok_or("Không tìm thấy Codex CLI (codex). Hãy cài Codex và đăng nhập bằng tài khoản ChatGPT, hoặc chỉ đường dẫn trong Cài đặt → AI.")?;
    let out_file = work.join(format!("codex-{}.txt", crate::util::random_id(6)));
    let mut args: Vec<String> = [
        "exec",
        "--skip-git-repo-check",
        "--ephemeral",
        "--sandbox",
        "read-only",
        "--color",
        "never",
        "-o",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(out_file.to_string_lossy().into_owned());
    let model = req.choice.model.trim();
    if !model.is_empty() && model != "default" {
        args.push("-m".into());
        args.push(model.into());
    }
    let effort = if req.choice.thinking {
        effort_or(req.choice, "medium")
    } else {
        "minimal".into()
    };
    args.push("-c".into());
    args.push(format!("model_reasoning_effort={effort}"));
    args.push("-".into());
    let input = format!(
        "{}\n\nDo not run commands or edit files. Reply with the answer only.\n\n{}",
        req.system, req.user
    );
    let (code, stdout, stderr) = run_cli(&program, &args, &input, &[], work)?;
    let answer = std::fs::read_to_string(&out_file).unwrap_or_default();
    let _ = std::fs::remove_file(&out_file);
    if !answer.trim().is_empty() {
        return Ok(answer.trim().to_string());
    }
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    Err(format!(
        "Codex dừng với mã {code}: {}",
        crate::util::truncate_chars(detail, 600)
    ))
}

fn api_error(status: reqwest::StatusCode, body: &str) -> String {
    let msg = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.pointer("/0/error/message"))
                .or_else(|| v.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| crate::util::truncate_chars(body, 400));
    format!("HTTP {status}: {msg}")
}

fn content_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

async fn openai_compatible(
    http: &reqwest::Client,
    p: &ProviderConfig,
    key: Option<&str>,
    req: &Request<'_>,
) -> Result<String, String> {
    let model = req.choice.model.trim();
    if model.is_empty() {
        return Err(format!(
            "Hãy chọn model cho {} (bấm \"Tải danh sách model\" trong Cài đặt → AI).",
            p.name
        ));
    }
    let url = format!("{}/chat/completions", p.base_url.trim_end_matches('/'));
    let mut body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": req.system },
            { "role": "user", "content": req.user }
        ],
    });
    let effort = match (p.kind, req.choice.thinking) {
        (_, true) => Some(effort_or(req.choice, "medium")),
        (ProviderKind::Gemini, false) => Some("none".to_string()),
        (ProviderKind::Openai, false) => Some("minimal".to_string()),
        _ => None,
    };
    if let Some(e) = &effort {
        body["reasoning_effort"] = json!(e);
    }
    for attempt in 0..2 {
        let mut rb = http.post(&url).timeout(HTTP_TIMEOUT).json(&body);
        if let Some(k) = key {
            rb = rb.bearer_auth(k);
        }
        let resp = rb.send().await.map_err(|e| format!("{}: {e}", p.name))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if status.is_success() {
            let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.name))?;
            return Ok(content_text(
                v.pointer("/choices/0/message/content")
                    .unwrap_or(&Value::Null),
            ));
        }
        // Some models refuse the effort setting: ask once more without it.
        if attempt == 0
            && status.as_u16() == 400
            && body.get("reasoning_effort").is_some()
            && text.to_lowercase().contains("reasoning")
        {
            if let Some(o) = body.as_object_mut() {
                o.remove("reasoning_effort");
            }
            continue;
        }
        return Err(format!("{}: {}", p.name, api_error(status, &text)));
    }
    Err(format!("{}: request failed", p.name))
}

async fn anthropic(
    http: &reqwest::Client,
    p: &ProviderConfig,
    key: Option<&str>,
    req: &Request<'_>,
) -> Result<String, String> {
    let key = key.ok_or("Chưa có Anthropic API key")?;
    let model = req.choice.model.trim();
    if model.is_empty() {
        return Err(
            "Hãy chọn model Anthropic (bấm \"Tải danh sách model\" trong Cài đặt → AI).".into(),
        );
    }
    let budget = match req.choice.effort.as_str() {
        "low" => 2048,
        "high" | "xhigh" | "max" => 16000,
        _ => 6000,
    };
    let mut body = json!({
        "model": model,
        "max_tokens": if req.choice.thinking { budget + 4096 } else { 4096 },
        "system": req.system,
        "messages": [{ "role": "user", "content": req.user }],
    });
    if req.choice.thinking {
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
    }
    let url = format!("{}/v1/messages", p.base_url.trim_end_matches('/'));
    for attempt in 0..2 {
        let resp = http
            .post(&url)
            .timeout(HTTP_TIMEOUT)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Anthropic: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if status.is_success() {
            let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            let out = v
                .get("content")
                .and_then(|c| c.as_array())
                .map(|parts| {
                    parts
                        .iter()
                        .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();
            return Ok(out);
        }
        if attempt == 0
            && status.as_u16() == 400
            && body.get("thinking").is_some()
            && text.contains("thinking")
        {
            if let Some(o) = body.as_object_mut() {
                o.remove("thinking");
                o.insert("max_tokens".into(), json!(4096));
            }
            continue;
        }
        return Err(format!("Anthropic: {}", api_error(status, &text)));
    }
    Err("Anthropic: request failed".into())
}

/// Sends one request to the chosen provider and returns its text answer.
pub async fn complete(
    http: &reqwest::Client,
    p: &ProviderConfig,
    key: Option<String>,
    req: Request<'_>,
    work: &Path,
) -> Result<String, String> {
    if !p.enabled {
        return Err(format!("{} đang tắt", p.name));
    }
    if p.kind.uses_key() && key.is_none() && p.kind != ProviderKind::Compatible {
        return Err(format!("Chưa nhập API key cho {} (Cài đặt → AI)", p.name));
    }
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    match p.kind {
        ProviderKind::ClaudeCode | ProviderKind::CodexCli => {
            let (p, system, user, choice, work) = (
                p.clone(),
                req.system.to_string(),
                req.user.to_string(),
                req.choice.clone(),
                work.to_path_buf(),
            );
            tauri::async_runtime::spawn_blocking(move || {
                let r = Request {
                    system: &system,
                    user: &user,
                    choice: &choice,
                };
                if p.kind == ProviderKind::ClaudeCode {
                    claude_code(&p, &r, &work)
                } else {
                    codex(&p, &r, &work)
                }
            })
            .await
            .map_err(|e| e.to_string())?
        }
        ProviderKind::Anthropic => anthropic(http, p, key.as_deref(), &req).await,
        _ => openai_compatible(http, p, key.as_deref(), &req).await,
    }
}

/// Models offered by a provider (API providers list them; the CLIs take aliases).
pub async fn list_models(
    http: &reqwest::Client,
    p: &ProviderConfig,
    key: Option<String>,
) -> Result<Vec<String>, String> {
    match p.kind {
        ProviderKind::ClaudeCode => Ok(vec!["opus".into(), "sonnet".into(), "haiku".into()]),
        ProviderKind::CodexCli => Ok(p.models.clone()),
        ProviderKind::Anthropic => {
            let key = key.ok_or("Chưa có Anthropic API key")?;
            let resp = http
                .get(format!(
                    "{}/v1/models?limit=100",
                    p.base_url.trim_end_matches('/')
                ))
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .timeout(Duration::from_secs(30))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(api_error(status, &text));
            }
            let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            Ok(ids(&v))
        }
        _ => {
            let mut rb = http
                .get(format!("{}/models", p.base_url.trim_end_matches('/')))
                .timeout(Duration::from_secs(30));
            if let Some(k) = key {
                rb = rb.bearer_auth(k);
            }
            let resp = rb.send().await.map_err(|e| e.to_string())?;
            let status = resp.status();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(api_error(status, &text));
            }
            let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            let mut out = ids(&v);
            if p.kind == ProviderKind::Gemini {
                out = out
                    .into_iter()
                    .map(|m| m.trim_start_matches("models/").to_string())
                    .collect();
            }
            Ok(out)
        }
    }
}

fn ids(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v
        .get("data")
        .or_else(|| v.get("models"))
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    m.get("id")
                        .or_else(|| m.get("name"))
                        .and_then(|i| i.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_from_listings() {
        let v = json!({"data": [{"id": "b"}, {"id": "a"}, {"id": "a"}]});
        assert_eq!(ids(&v), vec!["a", "b"]);
        let v = json!({"models": [{"name": "models/gemini-x"}]});
        assert_eq!(ids(&v), vec!["models/gemini-x"]);
    }

    #[test]
    fn content_parts() {
        assert_eq!(
            content_text(&json!([{"type":"text","text":"a"},{"type":"text","text":"b"}])),
            "ab"
        );
        assert_eq!(content_text(&json!("x")), "x");
    }

    #[cfg(unix)]
    #[test]
    fn cli_runner_passes_stdin_and_times_out_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let (code, out, _) =
            run_cli(Path::new("/bin/cat"), &[], "xin chào", &[], dir.path()).unwrap();
        assert_eq!(code, 0);
        assert_eq!(out, "xin chào");
    }
}
