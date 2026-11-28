/**
 * Wire Desk — TypeScript unit tests: security & scope (specs #218).
 * Runs offline with no external network or API keys.
 */
import assert from "node:assert/strict";

/* ------------------------------------------------------------------ */
/* Scope allowlist mirroring Rust `security.rs`                       */
/* ------------------------------------------------------------------ */

function domainInScope(host: string, wildcardDomains: string[]): boolean {
  const h = host.toLowerCase();
  for (const wd of wildcardDomains) {
    const d = wd.toLowerCase().replace(/^\*\./, "");
    if (h === d || h.endsWith("." + d)) return true;
  }
  return false;
}

function parseCidr(cidr: string): { ip: string; prefix: number } | null {
  const parts = cidr.split("/");
  if (parts.length !== 2) return null;
  const prefix = Number(parts[1]);
  if (!Number.isInteger(prefix) || prefix < 0 || prefix > 128) return null;
  return { ip: parts[0], prefix };
}

function ipInCidr(ip: string, net: { ip: string; prefix: number }): boolean {
  // Minimal IPv4 check.
  const ipParts = ip.split(".").map(Number);
  const netParts = net.ip.split(".").map(Number);
  if (ipParts.some((p) => p < 0 || p > 255) || netParts.some((p) => p < 0 || p > 255)) return false;
  let ipInt = (ipParts[0] << 24) | (ipParts[1] << 16) | (ipParts[2] << 8) | ipParts[3];
  let netInt = (netParts[0] << 24) | (netParts[1] << 16) | (netParts[2] << 8) | netParts[3];
  const mask = net.prefix === 0 ? 0 : (~0 << (32 - net.prefix)) >>> 0;
  return (ipInt & mask) === (netInt & mask);
}

function ssrfBlocked(host: string): boolean {
  // Block localhost, private ranges, metadata.
  if (host === "127.0.0.1" || host === "::1" || host === "localhost") return true;
  const m = host.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)$/);
  if (m) {
    const [, a, b, c, d] = m.map(Number);
    if (a === 127) return true;
    if (a === 10) return true;
    if (a === 172 && b >= 16 && b <= 31) return true;
    if (a === 192 && b === 168) return true;
    if (a === 169 && b === 254 && c === 169 && d === 254) return true; // metadata
    return false; // public IPv4
  }
  return false; // hostname: defer to resolver
}

/* ------------------------------------------------------------------ */
/* Tests                                                              */
/* ------------------------------------------------------------------ */

// Wildcard domain matching.
assert.ok(domainInScope("sub.example.com", ["*.example.com"]), "wildcard matches subdomain");
assert.ok(domainInScope("example.com", ["*.example.com"]), "wildcard matches apex");
assert.ok(!domainInScope("evil.com", ["*.example.com"]), "wildcard rejects unrelated domain");

// Exact domain.
assert.ok(domainInScope("example.com", []), "exact handled by caller (placeholder)");
assert.ok(!domainInScope("other.com", ["*.example.com"]), "no wildcard for other.com");

// CIDR IPv4.
assert.ok(ipInCidr("10.0.0.5", { ip: "10.0.0.0", prefix: 8 }), "10/8 contains 10.0.0.5");
assert.ok(!ipInCidr("8.8.8.8", { ip: "10.0.0.0", prefix: 8 }), "10/8 excludes 8.8.8.8");
assert.ok(ipInCidr("172.16.5.5", { ip: "172.16.0.0", prefix: 12 }), "172.16/12 contains");
assert.ok(!ipInCidr("172.32.5.5", { ip: "172.16.0.0", prefix: 12 }), "172.16/12 excludes 172.32");

// SSRF blocks.
assert.ok(ssrfBlocked("127.0.0.1"), "blocks localhost");
assert.ok(ssrfBlocked("10.0.0.1"), "blocks 10/8");
assert.ok(ssrfBlocked("172.16.0.1"), "blocks 172.16/12");
assert.ok(ssrfBlocked("192.168.1.1"), "blocks 192.168/16");
assert.ok(ssrfBlocked("169.254.169.254"), "blocks cloud metadata");
assert.ok(!ssrfBlocked("8.8.8.8"), "allows public IP");
assert.ok(!ssrfBlocked("example.com"), "defers hostname to resolver");

console.log("security tests passed");
