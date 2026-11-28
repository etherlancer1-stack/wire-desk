//! Defensive security guards: scope allowlist, SSRF/IP validation, path
//! traversal protection, secret redaction (specs #121-127, #144-147, #159).
//!
//! Deliberately dependency-free (no `regex`, no `url` crate) to keep the
//! dependency tree lean per spec #25.

use crate::error::WireError;
use crate::error::WireResult;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Component, Path, PathBuf};

/// Parsed scope allowlist (spec #121). Matches `scope_allowlist_json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScopeAllowlist {
    pub domains: Vec<String>,
    pub wildcard_domains: Vec<String>,
    pub cidrs: Vec<String>,
    pub ips: Vec<String>,
    pub paths: Vec<String>,
    #[serde(default)]
    pub allow_private_ips: bool,
}

impl ScopeAllowlist {
    pub fn from_json(json: &str) -> WireResult<Self> {
        serde_json::from_str(json).map_err(|e| {
            WireError::Validation(format!("invalid scope_allowlist_json: {}", e))
        })
    }

    /// Check whether a target host/IP is within scope.
    pub fn is_in_scope(&self, target: &str) -> bool {
        if let Ok(ip) = target.parse::<IpAddr>() {
            return self.ip_in_scope(ip);
        }
        self.domain_in_scope(target)
    }

    fn domain_in_scope(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        if self.domains.iter().any(|d| d.to_ascii_lowercase() == host) {
            return true;
        }
        for wd in &self.wildcard_domains {
            let wd = wd.to_ascii_lowercase();
            let wd = wd.trim_start_matches("*.");
            if host == wd || host.ends_with(&format!(".{}", wd)) {
                return true;
            }
        }
        false
    }

    fn ip_in_scope(&self, ip: IpAddr) -> bool {
        if self.ips.iter().any(|s| s.parse::<IpAddr>().ok() == Some(ip)) {
            return true;
        }
        for cidr in &self.cidrs {
            if let Some(net) = parse_cidr(cidr) {
                if net.contains(ip) {
                    return true;
                }
            }
        }
        self.allow_private_ips && ip.is_private()
    }
}

/// Minimal CIDR network matcher (IPv4 + IPv6).
struct CidrNet {
    network: IpAddr,
    prefix: u8,
    is_v4: bool,
}

impl CidrNet {
    fn contains(&self, ip: IpAddr) -> bool {
        if self.is_v4 != ip.is_ipv4() {
            return false;
        }
        match (self.network, ip) {
            (IpAddr::V4(a), IpAddr::V4(b)) => {
                let mask: u32 = !0u32 << (32 - self.prefix);
                (a.to_bits() & mask) == (b.to_bits() & mask)
            }
            (IpAddr::V6(a), IpAddr::V6(b)) => {
                let mask: u128 = !0u128 << (128 - self.prefix);
                (a.to_bits() & mask) == (b.to_bits() & mask)
            }
            _ => false,
        }
    }
}

fn parse_cidr(cidr: &str) -> Option<CidrNet> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let ip: IpAddr = parts[0].trim().parse().ok()?;
    let prefix: u8 = parts[1].trim().parse().ok()?;
    let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
    if prefix > max_prefix {
        return None;
    }
    Some(CidrNet { network: ip, prefix, is_v4: ip.is_ipv4() })
}

/// SSRF target validator (specs #125, #126).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsrfVerdict {
    Allowed,
    BlockedScheme,
    BlockedPrivateIp,
    BlockedLocalhost,
    BlockedMetadata,
    BlockedReserved,
}

pub struct SsrfValidator;

impl SsrfValidator {
    /// Validate a URL before connecting. Returns the verdict.
    pub fn validate(input: &str) -> SsrfVerdict {
        let parsed = match parse_url(input) {
            Some(u) => u,
            None => return SsrfVerdict::BlockedScheme,
        };
        let scheme = parsed.scheme.as_str();
        if scheme != "http" && scheme != "https" {
            return SsrfVerdict::BlockedScheme;
        }
        let host = parsed.host.as_str();
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Self::check_ip(ip);
        }
        // Hostname: allow here; the async fetcher must resolve and re-check
        // every resolved IP (spec #126) before connecting.
        SsrfVerdict::Allowed
    }

    fn check_ip(ip: IpAddr) -> SsrfVerdict {
        if ip.is_loopback() {
            return SsrfVerdict::BlockedLocalhost;
        }
        // Cloud metadata (IPv4 169.254.169.254).
        if ip == IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)) {
            return SsrfVerdict::BlockedMetadata;
        }
        // IPv4 private ranges: 10/8, 172.16/12, 192.168/16, 127/8.
        if ip.is_ipv4() {
            if let IpAddr::V4(v4) = ip {
                let o = v4.octets();
                if o[0] == 10
                    || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)
                    || (o[0] == 192 && o[1] == 168)
                    || o[0] == 127
                {
                    return SsrfVerdict::BlockedPrivateIp;
                }
            }
        }
        // IPv6: ::1, fe80::/10, fc00::/7.
        if ip.is_ipv6() {
            if let IpAddr::V6(v6) = ip {
                let segs = v6.segments();
                if v6 == Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1) {
                    return SsrfVerdict::BlockedLocalhost;
                }
                if (segs[0] & 0xffc0) == 0xfe80 {
                    return SsrfVerdict::BlockedPrivateIp;
                }
                if (segs[0] & 0xfe00) == 0xfc00 {
                    return SsrfVerdict::BlockedPrivateIp;
                }
            }
        }
        SsrfVerdict::Allowed
    }
}

/// Tiny URL parser for `http://` / `https://` URLs.
struct TinyUrl {
    scheme: String,
    host: String,
    path: String,
}

fn parse_url(input: &str) -> Option<TinyUrl> {
    let (scheme, rest) = input.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (host, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let host = host.split(':').next().unwrap_or(host).to_string();
    Some(TinyUrl { scheme: scheme.to_string(), host, path: path.to_string() })
}

/// Path canonicalization guard (spec #145).
pub fn is_path_within_root(path: &Path, root: &Path) -> WireResult<bool> {
    let canonical_root = root.canonicalize().map_err(|e| {
        WireError::Validation(format!("cannot canonicalize root {:?}: {}", root, e))
    })?;
    let canonical_path = if path.exists() {
        path.canonicalize().map_err(|e| {
            WireError::Validation(format!("cannot canonicalize path {:?}: {}", path, e))
        })?
    } else {
        let parent = path.parent().unwrap_or(root);
        let canonical_parent = parent.canonicalize().map_err(|e| {
            WireError::Validation(format!("cannot canonicalize parent {:?}: {}", parent, e))
        })?;
        canonical_parent.join(path.file_name().unwrap_or_default())
    };
    Ok(canonical_path.starts_with(&canonical_root))
}

/// Reject path traversal attempts (spec #145).
pub fn contains_traversal(path: &Path) -> bool {
    path.components().any(|c| matches!(c, Component::ParentDir))
}

/// Secret redaction (spec #146).
pub fn redact_secrets(input: &str) -> String {
    let mut out = input.to_string();
    out = redact_bearer(&out);
    for prefix in [
        "sk_live_", "sk_test_", "whsec_", "wd_live_", "sk-", "ghp_",
        "gho_", "xoxb-", "sk-or-",
    ] {
        out = out.replace(prefix, "[REDACTED]");
    }
    out
}

fn redact_bearer(input: &str) -> String {
    let mut out = input.to_string();
    let marker = "Bearer ";
    let mut start = 0;
    while let Some(idx) = out[start..].find(marker) {
        let abs = start + idx + marker.len();
        let end = out[abs..]
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '}')
            .map(|e| abs + e)
            .unwrap_or(out.len());
        out.replace_range(abs..end, "[REDACTED]");
        start = abs;
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsrfBlockedEvent {
    pub url: String,
    pub verdict: String,
    pub timestamp: String,
}

impl SsrfBlockedEvent {
    pub fn new(url: &str, verdict: SsrfVerdict) -> Self {
        Self {
            url: url.to_string(),
            verdict: format!("{:?}", verdict),
            timestamp: crate::models::utc_now(),
        }
    }
}
