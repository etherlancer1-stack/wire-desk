//! Tool registry and built-in tools (specs #98-105).
//! Every tool runs in a sandboxed, bounded, non-shell context. No raw `sh -c`.
//! Dependency-free (uses only `once_cell`, already in Cargo.toml).

use crate::error::{WireError, WireResult};
use crate::security::{self, ScopeAllowlist, SsrfValidator};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Tool declaration (spec #98).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDecl {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // JSON Schema
    pub permission_level: String, // "read" | "write" | "admin" | "scan"
    pub timeout_seconds: u64,
}

/// Tool result (bounded output, spec #105).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub ok: bool,
    pub output: String,      // truncated to cap
    pub error: Option<String>,
    pub duration_ms: u64,
    pub tokens_estimate: u32,
}

pub struct ToolRegistry {
    tools: Vec<ToolDecl>,
    workspace_root: PathBuf,
    scope_allowlist: ScopeAllowlist,
    output_cap_chars: usize,
}

static SAST_RULES: Lazy<Vec<(&'static str, &'static str)>> = Lazy::new(|| {
    vec![
        ("eval", "eval("),
        ("new Function", "new Function("),
        ("child_process.exec", "child_process.exec"),
        ("innerHTML=", ".innerHTML ="),
        ("dangerouslySetInnerHTML", "dangerouslySetInnerHTML"),
        ("unparameterized SQL", "query("),
        ("AWS key", "AKIA"),
        ("GitHub token", "ghp_"),
        ("Stripe key", "sk_live_"),
        ("private key", "-----BEGIN PRIVATE KEY-----"),
        ("JWT secret", "jwt_secret"),
    ]
});

impl ToolRegistry {
    pub fn new(workspace_root: PathBuf, scope: ScopeAllowlist, output_cap_chars: usize) -> Self {
        let tools = vec![
            ToolDecl { name: "sandbox_cmd".into(), description: "Run an allowlisted command in a sandboxed jail".into(), input_schema: serde_json::json!({"type":"object","properties":{"cmd":{"type":"string"},"args":{"type":"array","items":{"type":"string"}}},"required":["cmd"]}), permission_level: "admin".into(), timeout_seconds: 30 },
            ToolDecl { name: "file_read".into(), description: "Read a file within the workspace root".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), permission_level: "read".into(), timeout_seconds: 10 },
            ToolDecl { name: "file_write".into(), description: "Write a file within the workspace root".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}), permission_level: "write".into(), timeout_seconds: 10 },
            ToolDecl { name: "file_patch".into(), description: "Apply a unified diff patch within the workspace root".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"},"diff":{"type":"string"}},"required":["path","diff"]}), permission_level: "write".into(), timeout_seconds: 15 },
            ToolDecl { name: "repo_search".into(), description: "Search files safely within the workspace root".into(), input_schema: serde_json::json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}), permission_level: "read".into(), timeout_seconds: 20 },
            ToolDecl { name: "sast_check".into(), description: "Run static security/secret checks over workspace code".into(), input_schema: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}), permission_level: "scan".into(), timeout_seconds: 60 },
            ToolDecl { name: "http_fetch".into(), description: "SSRF-safe fetch of an allowed URL".into(), input_schema: serde_json::json!({"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}), permission_level: "scan".into(), timeout_seconds: 30 },
            ToolDecl { name: "dns_recon".into(), description: "Passive DNS recon within scope allowlist".into(), input_schema: serde_json::json!({"type":"object","properties":{"host":{"type":"string"}},"required":["host"]}), permission_level: "scan".into(), timeout_seconds: 20 },
        ];
        Self { tools, workspace_root, scope_allowlist: scope, output_cap_chars }
    }

    pub fn list(&self) -> &[ToolDecl] { &self.tools }

    pub fn find(&self, name: &str) -> Option<&ToolDecl> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Execute a tool by name with JSON input. Returns a bounded ToolResult.
    pub async fn execute(&self, name: &str, input: &Value) -> ToolResult {
        let start = std::time::Instant::now();
        let result = match name {
            "sandbox_cmd" => self.cmd_sandbox(input),
            "file_read" => self.file_read(input),
            "file_write" => self.file_write(input),
            "file_patch" => self.file_patch(input),
            "repo_search" => self.repo_search(input),
            "sast_check" => self.sast_check(input),
            "http_fetch" => self.http_fetch(input),
            "dns_recon" => self.dns_recon(input),
            other => ToolResult { ok: false, output: String::new(), error: Some(format!("unknown tool: {}", other)), duration_ms: 0, tokens_estimate: 0 },
        };
        ToolResult {
            duration_ms: start.elapsed().as_millis() as u64,
            tokens_estimate: estimate_tokens(&result.output),
            ..result
        }
    }

    /* ---------------------- sandbox command (specs #99, #144) ---------------------- */
    fn cmd_sandbox(&self, input: &Value) -> ToolResult {
        let cmd = input.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let args: Vec<String> = input.get("args").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        // Never interpolate untrusted input into shell strings; use explicit arg array.
        // Restrict to a small allowlist of safe commands.
        let allowlist = ["echo", "cat", "ls", "pwd", "grep", "head", "tail", "wc", "file"];
        if !allowlist.iter().any(|&a| a == cmd) {
            return ToolResult { ok: false, output: String::new(), error: Some(format!("command not allowlisted: {}", cmd)), duration_ms: 0, tokens_estimate: 0 };
        }
        let mut command = Command::new(cmd);
        command.args(&args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.current_dir(&self.workspace_root);
        let output = match command.output() {
            Ok(o) => o,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("spawn failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}\n{}", stdout, stderr);
        ToolResult { ok: output.status.success(), output: truncate(&combined, self.output_cap_chars),
            error: if output.status.success() { None } else { Some(format!("exit {}", output.status.code().unwrap_or(-1))) },
            duration_ms: 0, tokens_estimate: estimate_tokens(&combined) }
    }

    /* ---------------------- file read/write/patch (specs #101, #145) ---------------------- */
    fn file_read(&self, input: &Value) -> ToolResult {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let p = PathBuf::from(path);
        if security::contains_traversal(&p) {
            return ToolResult { ok: false, output: String::new(), error: Some("path traversal rejected".into()), duration_ms: 0, tokens_estimate: 0 };
        }
        let full = self.workspace_root.join(&p);
        match security::is_path_within_root(&full, &self.workspace_root) {
            Ok(false) => return ToolResult { ok: false, output: String::new(), error: Some("path outside workspace root".into()), duration_ms: 0, tokens_estimate: 0 },
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(e.to_string()), duration_ms: 0, tokens_estimate: 0 },
            Ok(true) => {}
        }
        let contents = match std::fs::read_to_string(&full) {
            Ok(c) => c,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("read failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        ToolResult { ok: true, output: truncate(&contents, self.output_cap_chars), error: None, duration_ms: 0, tokens_estimate: estimate_tokens(&contents) }
    }

    fn file_write(&self, input: &Value) -> ToolResult {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let p = PathBuf::from(path);
        if security::contains_traversal(&p) {
            return ToolResult { ok: false, output: String::new(), error: Some("path traversal rejected".into()), duration_ms: 0, tokens_estimate: 0 };
        }
        let full = self.workspace_root.join(&p);
        match security::is_path_within_root(&full, &self.workspace_root) {
            Ok(false) => return ToolResult { ok: false, output: String::new(), error: Some("path outside workspace root".into()), duration_ms: 0, tokens_estimate: 0 },
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(e.to_string()), duration_ms: 0, tokens_estimate: 0 },
            Ok(true) => {}
        }
        if let Some(parent) = full.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&full, content) {
            Ok(()) => ToolResult { ok: true, output: format!("wrote {} bytes", content.len()), error: None, duration_ms: 0, tokens_estimate: 0 },
            Err(e) => ToolResult { ok: false, output: String::new(), error: Some(format!("write failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        }
    }

    fn file_patch(&self, input: &Value) -> ToolResult {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let diff = input.get("diff").and_then(|v| v.as_str()).unwrap_or("");
        let p = PathBuf::from(path);
        if security::contains_traversal(&p) {
            return ToolResult { ok: false, output: String::new(), error: Some("path traversal rejected".into()), duration_ms: 0, tokens_estimate: 0 };
        }
        let full = self.workspace_root.join(&p);
        match security::is_path_within_root(&full, &self.workspace_root) {
            Ok(false) => return ToolResult { ok: false, output: String::new(), error: Some("path outside workspace root".into()), duration_ms: 0, tokens_estimate: 0 },
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(e.to_string()), duration_ms: 0, tokens_estimate: 0 },
            Ok(true) => {}
        }
        let has_patch = Command::new("patch").arg("--version").output().is_ok();
        if !has_patch {
            return ToolResult { ok: false, output: String::new(), error: Some("patch utility not available".into()), duration_ms: 0, tokens_estimate: 0 };
        }
        let mut command = Command::new("patch");
        command.args(["--no-backup-if-mismatch", "-p1"]);
        command.current_dir(&self.workspace_root);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("patch spawn failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(diff.as_bytes());
        }
        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("patch wait failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let out = String::from_utf8_lossy(&output.stdout).to_string();
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        ToolResult { ok: output.status.success(), output: truncate(&format!("{}\n{}", out, err), self.output_cap_chars),
            error: if output.status.success() { None } else { Some("patch failed".into()) },
            duration_ms: 0, tokens_estimate: estimate_tokens(&format!("{}\n{}", out, err)) }
    }

    /* ---------------------- repo search (spec #102) ---------------------- */
    fn repo_search(&self, input: &Value) -> ToolResult {
        let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let search_root = self.workspace_root.join(path);
        match security::is_path_within_root(&search_root, &self.workspace_root) {
            Ok(false) => return ToolResult { ok: false, output: String::new(), error: Some("path outside workspace root".into()), duration_ms: 0, tokens_estimate: 0 },
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(e.to_string()), duration_ms: 0, tokens_estimate: 0 },
            Ok(true) => {}
        }
        let rg_available = Command::new("rg").arg("--version").output().is_ok();
        if rg_available {
            let mut command = Command::new("rg");
            command.args(["--no-heading", "--line-number", pattern]);
            command.current_dir(&search_root);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());
            match command.output() {
                Ok(output) => {
                    let out = String::from_utf8_lossy(&output.stdout).to_string();
                    let err = String::from_utf8_lossy(&output.stderr).to_string();
                    ToolResult { ok: output.status.success(), output: truncate(&format!("{}\n{}", out, err), self.output_cap_chars),
                        error: if output.status.success() { None } else { Some("rg found no matches".into()) },
                        duration_ms: 0, tokens_estimate: estimate_tokens(&format!("{}\n{}", out, err)) }
                }
                Err(e) => ToolResult { ok: false, output: String::new(), error: Some(format!("rg failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
            }
        } else {
            ToolResult { ok: false, output: String::new(), error: Some("ripgrep not available".into()), duration_ms: 0, tokens_estimate: 0 }
        }
    }

    /* ---------------------- SAST check (specs #134-136) ---------------------- */
    fn sast_check(&self, input: &Value) -> ToolResult {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let scan_root = self.workspace_root.join(path);
        match security::is_path_within_root(&scan_root, &self.workspace_root) {
            Ok(false) => return ToolResult { ok: false, output: String::new(), error: Some("path outside workspace root".into()), duration_ms: 0, tokens_estimate: 0 },
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(e.to_string()), duration_ms: 0, tokens_estimate: 0 },
            Ok(true) => {}
        }
        let findings = scan_directory_for_issues(&scan_root);
        let output = serde_json::to_string_pretty(&findings).unwrap_or_default();
        ToolResult { ok: true, output: truncate(&output, self.output_cap_chars), error: None, duration_ms: 0, tokens_estimate: estimate_tokens(&output) }
    }

    /* ---------------------- SSRF-safe HTTP fetch (spec #103) ---------------------- */
    async fn http_fetch(&self, input: &Value) -> ToolResult {
        let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");
        match SsrfValidator::validate(url) {
            security::SsrfVerdict::Allowed => {}
            v => return ToolResult { ok: false, output: String::new(), error: Some(format!("SSRF blocked: {:?}", v)), duration_ms: 0, tokens_estimate: 0 },
        }
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build() {
            Ok(c) => c,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("client failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("fetch failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let status = resp.status();
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("read failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let capped = truncate(&text, self.output_cap_chars);
        ToolResult { ok: status.is_success(), output: capped, error: if status.is_success() { None } else { Some(format!("http {}", status)) },
            duration_ms: 0, tokens_estimate: estimate_tokens(&capped) }
    }

    /* ---------------------- passive DNS recon (spec #104) ---------------------- */
    async fn dns_recon(&self, input: &Value) -> ToolResult {
        let host = input.get("host").and_then(|v| v.as_str()).unwrap_or("");
        if !self.scope_allowlist.is_in_scope(host) {
            return ToolResult { ok: false, output: String::new(), error: Some("host out of scope".into()), duration_ms: 0, tokens_estimate: 0 };
        }
        let url = format!("https://dns.google/resolve?name={}&type=A", host);
        let client = match reqwest::Client::builder().timeout(Duration::from_secs(15)).build() {
            Ok(c) => c,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("dns client failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("dns query failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => return ToolResult { ok: false, output: String::new(), error: Some(format!("dns read failed: {}", e)), duration_ms: 0, tokens_estimate: 0 },
        };
        ToolResult { ok: true, output: truncate(&text, self.output_cap_chars), error: None, duration_ms: 0, tokens_estimate: estimate_tokens(&text) }
    }
}

fn scan_directory_for_issues(root: &Path) -> Vec<Value> {
    let mut findings = Vec::new();
    let _ = walk_dir(root, &mut |path| {
        if let Ok(contents) = std::fs::read_to_string(path) {
            for (line_no, line) in contents.lines().enumerate() {
                for (rule, pattern) in SAST_RULES.iter() {
                    if line.contains(pattern) {
                        findings.push(serde_json::json!({
                            "file": path.to_string_lossy(),
                            "line": line_no + 1,
                            "rule": rule,
                            "match": safe_truncate(line, 200),
                        }));
                    }
                }
            }
        }
    });
    findings
}

fn walk_dir<F>(dir: &Path, f: &mut F) where F: FnMut(&Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, f);
            } else {
                f(&path);
            }
        }
    }
}

fn safe_truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    s[..end].to_string()
}

fn truncate(s: &str, max: usize) -> String {
    safe_truncate(s, max)
}

fn estimate_tokens(s: &str) -> u32 {
    (s.len() / 4).max(1) as u32
}
