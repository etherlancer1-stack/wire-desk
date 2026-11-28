/**
 * Wire Desk — TypeScript unit tests: billing, quotas & auth (specs #219).
 * Offline, deterministic.
 */
import assert from "node:assert/strict";

const PLANS: Record<string, { tokenQuota: number; scanQuota: number }> = {
  free: { tokenQuota: 50_000, scanQuota: 100 },
  pro: { tokenQuota: 1_500_000, scanQuota: 200 },
  team: { tokenQuota: 7_500_000, scanQuota: 1_000 },
  enterprise: { tokenQuota: 50_000_000, scanQuota: 10_000 },
};

function quotaOk(tier: string, usedRuns: number, usedScans: number, reqRuns: number, reqScans: number): boolean {
  const p = PLANS[tier];
  if (!p) return false;
  return usedRuns + reqRuns <= p.tokenQuota / 100 && usedScans + reqScans <= p.scanQuota;
}

// Cost math: per-1M rates.
function costUsd(tokensIn: number, tokensOut: number, rateIn = 1.0, rateOut = 2.0): number {
  return (tokensIn / 1_000_000) * rateIn + (tokensOut / 1_000_000) * rateOut;
}

assert.ok(quotaOk("free", 0, 0, 1, 1), "free tier allows a run+scan");
assert.ok(!quotaOk("free", 500, 0, 1, 0), "free tier rejects when run quota exceeded");
assert.ok(quotaOk("pro", 0, 0, 10, 10), "pro tier allows more");
assert.strictEqual(costUsd(1000, 500), 0.001 + 0.001, "cost math");
assert.strictEqual(costUsd(0, 0), 0, "zero cost");

// API key prefix.
function prefixOf(key: string): string { return key.slice(0, 8); }
assert.strictEqual(prefixOf("wd_live_abc123"), "wd_live_");
assert.strictEqual(prefixOf("sk_test_123"), "sk_test_");

console.log("billing tests passed");
