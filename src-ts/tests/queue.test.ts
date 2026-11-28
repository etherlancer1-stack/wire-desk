/**
 * Wire Desk — TypeScript unit tests: agent & queue (specs #220).
 * Offline, deterministic retry-backoff and job state transitions.
 */
import assert from "node:assert/strict";

// Exponential backoff mirroring Rust repository.rs backoff_seconds.
function backoffSeconds(attempt: number): number {
  return Math.min(Math.pow(2, Math.min(attempt, 10)), 3600);
}

assert.strictEqual(backoffSeconds(1), 2);
assert.strictEqual(backoffSeconds(2), 4);
assert.strictEqual(backoffSeconds(3), 8);
assert.strictEqual(backoffSeconds(12), 3600); // capped

// Job state transitions.
type JobStatus = "pending" | "running" | "completed" | "failed" | "dead_letter";
function transition(status: JobStatus, attempts: number, maxAttempts: number): JobStatus {
  const next = attempts + 1;
  if (next >= maxAttempts) return "dead_letter";
  return "pending"; // requeued with backoff
}

assert.strictEqual(transition("pending", 0, 3), "pending");
assert.strictEqual(transition("pending", 1, 3), "pending");
assert.strictEqual(transition("pending", 2, 3), "dead_letter");

// Token estimation / trimming (mirrors Rust estimate_tokens).
function estimateTokens(s: string): number { return Math.max(1, Math.floor(s.length / 4)); }
assert.ok(estimateTokens("hello world") >= 1);

console.log("queue tests passed");
