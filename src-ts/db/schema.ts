/**
 * Wire Desk — Drizzle SQL schema (SQLite dialect).
 *
 * Column names are snake_case to match the Rust persistence layer exactly.
 * TypeScript fields are camelCase (spec #37) and mapped to snake_case columns
 * via `columnName(...)`. Statuses, plan tiers, and job types align 1:1 with
 * migrations/0001_initial.sql and the Rust models in crates/wired/src/models.rs.
 */

import {
  sqliteTable,
  text,
  integer,
  real,
  index,
  unique,
} from "drizzle-orm/sqlite-core";

/* ------------------------------------------------------------------ */
/* Tenants                                                            */
/* ------------------------------------------------------------------ */
export const tenants = sqliteTable(
  "tenants",
  {
    id: text("id").primaryKey(),
    name: text("name").notNull(),
    slug: text("slug").notNull().unique(),
    planTier: text("plan_tier")
      .notNull()
      .default("free")
      .$type<"free" | "pro" | "team" | "enterprise">(),
    stripeCustomerId: text("stripe_customer_id"),
    stripeSubscriptionId: text("stripe_subscription_id"),
    monthlyTokenQuota: integer("monthly_token_quota").notNull().default(50000),
    monthlyScanQuota: integer("monthly_scan_quota").notNull().default(100),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (t) => [
    index("idx_tenants_slug").on(t.slug),
    index("idx_tenants_plan").on(t.planTier),
  ],
);

/* ------------------------------------------------------------------ */
/* Users                                                              */
/* ------------------------------------------------------------------ */
export const users = sqliteTable(
  "users",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    email: text("email").notNull(),
    role: text("role")
      .notNull()
      .default("viewer")
      .$type<"owner" | "admin" | "analyst" | "viewer">(),
    passwordHash: text("password_hash").notNull(),
    apiKeyPrefix: text("api_key_prefix").notNull().default("wd_live"),
    apiKeyHash: text("api_key_hash"),
    isActive: integer("is_active", { mode: "boolean" }).notNull().default(true),
    createdAt: text("created_at").notNull(),
  },
  (t) => [
    index("idx_users_tenant").on(t.tenantId),
    unique("idx_users_email_unique").on(t.email),
  ],
);

/* ------------------------------------------------------------------ */
/* Workspaces                                                         */
/* ------------------------------------------------------------------ */
export const workspaces = sqliteTable(
  "workspaces",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    name: text("name").notNull(),
    description: text("description").notNull().default(""),
    scopeAllowlistJson: text("scope_allowlist_json").notNull().default("[]"),
    rateLimitRps: real("rate_limit_rps").notNull().default(5.0),
    configJson: text("config_json").notNull().default("{}"),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (t) => [index("idx_workspaces_tenant").on(t.tenantId)],
);

/* ------------------------------------------------------------------ */
/* Agent runs                                                         */
/* ------------------------------------------------------------------ */
export const agentRuns = sqliteTable(
  "agent_runs",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    workspaceId: text("workspace_id")
      .notNull()
      .references(() => workspaces.id, { onDelete: "cascade" }),
    goal: text("goal").notNull(),
    status: text("status")
      .notNull()
      .default("pending")
      .$type<
        | "pending"
        | "running"
        | "waiting_approval"
        | "completed"
        | "failed"
        | "cancelled"
      >(),
    modelUsed: text("model_used"),
    iterations: integer("iterations").notNull().default(0),
    tokensIn: integer("tokens_in").notNull().default(0),
    tokensOut: integer("tokens_out").notNull().default(0),
    costUsd: real("cost_usd").notNull().default(0),
    errorMessage: text("error_message"),
    traceJson: text("trace_json").notNull().default("[]"),
    startedAt: text("started_at"),
    completedAt: text("completed_at"),
    createdAt: text("created_at").notNull(),
  },
  (t) => [
    index("idx_agent_runs_tenant").on(t.tenantId, t.createdAt),
    index("idx_agent_runs_status").on(t.status, t.createdAt),
    index("idx_agent_runs_workspace").on(t.workspaceId),
  ],
);

/* ------------------------------------------------------------------ */
/* Agent steps                                                        */
/* ------------------------------------------------------------------ */
export const agentSteps = sqliteTable(
  "agent_steps",
  {
    id: text("id").primaryKey(),
    runId: text("run_id")
      .notNull()
      .references(() => agentRuns.id, { onDelete: "cascade" }),
    stepIndex: integer("step_index").notNull(),
    phase: text("phase")
      .notNull()
      .$type<"plan" | "tool_call" | "tool_result" | "reflection" | "final">(),
    toolName: text("tool_name"),
    inputJson: text("input_json").notNull().default("{}"),
    outputJson: text("output_json").notNull().default("{}"),
    tokensUsed: integer("tokens_used").notNull().default(0),
    durationMs: integer("duration_ms").notNull().default(0),
    createdAt: text("created_at").notNull(),
  },
  (t) => [index("idx_agent_steps_run").on(t.runId, t.stepIndex)],
);

/* ------------------------------------------------------------------ */
/* Security scans                                                     */
/* ------------------------------------------------------------------ */
export const securityScans = sqliteTable(
  "security_scans",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    workspaceId: text("workspace_id")
      .notNull()
      .references(() => workspaces.id, { onDelete: "cascade" }),
    target: text("target").notNull(),
    scanType: text("scan_type")
      .notNull()
      .$type<
        | "passive_recon"
        | "dns_tls_audit"
        | "sast_repo"
        | "dep_audit"
        | "ssrf_check"
      >(),
    status: text("status")
      .notNull()
      .default("pending")
      .$type<"pending" | "running" | "completed" | "failed" | "cancelled">(),
    findingsCount: integer("findings_count").notNull().default(0),
    criticalCount: integer("critical_count").notNull().default(0),
    highCount: integer("high_count").notNull().default(0),
    startedAt: text("started_at"),
    completedAt: text("completed_at"),
    createdAt: text("created_at").notNull(),
  },
  (t) => [
    index("idx_security_scans_tenant").on(t.tenantId, t.createdAt),
    index("idx_security_scans_status").on(t.status, t.createdAt),
  ],
);

/* ------------------------------------------------------------------ */
/* Security findings                                                  */
/* ------------------------------------------------------------------ */
export const securityFindings = sqliteTable(
  "security_findings",
  {
    id: text("id").primaryKey(),
    scanId: text("scan_id")
      .notNull()
      .references(() => securityScans.id, { onDelete: "cascade" }),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    workspaceId: text("workspace_id")
      .notNull()
      .references(() => workspaces.id, { onDelete: "cascade" }),
    fingerprint: text("fingerprint").notNull(),
    category: text("category").notNull(),
    severity: text("severity")
      .notNull()
      .$type<"critical" | "high" | "medium" | "low" | "info">(),
    title: text("title").notNull(),
    description: text("description").notNull(),
    evidence: text("evidence").notNull().default(""),
    remediation: text("remediation").notNull().default(""),
    status: text("status")
      .notNull()
      .default("open")
      .$type<"open" | "triaged" | "resolved" | "false_positive">(),
    createdAt: text("created_at").notNull(),
  },
  (t) => [
    unique("idx_security_findings_fingerprint").on(
      t.workspaceId,
      t.fingerprint,
    ),
    index("idx_security_findings_scan").on(t.scanId),
    index("idx_security_findings_severity").on(
      t.tenantId,
      t.severity,
      t.createdAt,
    ),
  ],
);

/* ------------------------------------------------------------------ */
/* Jobs (durable queue)                                               */
/* ------------------------------------------------------------------ */
export const jobs = sqliteTable(
  "jobs",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    workspaceId: text("workspace_id")
      .notNull()
      .references(() => workspaces.id, { onDelete: "cascade" }),
    jobType: text("job_type")
      .notNull()
      .$type<"agent_run" | "security_scan" | "webhook_delivery" | "report_export">(),
    payloadJson: text("payload_json").notNull().default("{}"),
    status: text("status")
      .notNull()
      .default("pending")
      .$type<"pending" | "running" | "completed" | "failed" | "dead_letter">(),
    attempts: integer("attempts").notNull().default(0),
    maxAttempts: integer("max_attempts").notNull().default(3),
    leaseExpiresAt: text("lease_expires_at"),
    runAfterAt: text("run_after_at").notNull(),
    lastError: text("last_error"),
    createdAt: text("created_at").notNull(),
    updatedAt: text("updated_at").notNull(),
  },
  (t) => [
    index("idx_jobs_pending").on(t.status, t.runAfterAt),
    index("idx_jobs_tenant").on(t.tenantId, t.createdAt),
  ],
);

/* ------------------------------------------------------------------ */
/* Usage events (metering)                                            */
/* ------------------------------------------------------------------ */
export const usageEvents = sqliteTable(
  "usage_events",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    workspaceId: text("workspace_id")
      .notNull()
      .references(() => workspaces.id, { onDelete: "cascade" }),
    metricType: text("metric_type")
      .notNull()
      .$type<
        | "ai_tokens"
        | "ai_runs"
        | "scans"
        | "sandbox_sec"
        | "api_calls"
      >(),
    quantity: integer("quantity").notNull().default(0),
    costUsd: real("cost_usd").notNull().default(0),
    idempotencyKey: text("idempotency_key").notNull().unique(),
    recordedAt: text("recorded_at").notNull(),
  },
  (t) => [
    index("idx_usage_events_tenant").on(t.tenantId, t.recordedAt),
    index("idx_usage_events_idempotency").on(t.idempotencyKey),
  ],
);

/* ------------------------------------------------------------------ */
/* Webhook endpoints                                                  */
/* ------------------------------------------------------------------ */
export const webhookEndpoints = sqliteTable(
  "webhook_endpoints",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    url: text("url").notNull(),
    secretHash: text("secret_hash").notNull(),
    eventsJson: text("events_json").notNull().default("[]"),
    isActive: integer("is_active", { mode: "boolean" }).notNull().default(true),
    createdAt: text("created_at").notNull(),
  },
  (t) => [index("idx_webhook_endpoints_tenant").on(t.tenantId)],
);

/* ------------------------------------------------------------------ */
/* Audit logs                                                         */
/* ------------------------------------------------------------------ */
export const auditLogs = sqliteTable(
  "audit_logs",
  {
    id: text("id").primaryKey(),
    tenantId: text("tenant_id")
      .notNull()
      .references(() => tenants.id, { onDelete: "cascade" }),
    actorUserId: text("actor_user_id").references(() => users.id, {
      onDelete: "set null",
    }),
    action: text("action").notNull(),
    resourceType: text("resource_type").notNull(),
    resourceId: text("resource_id"),
    metadataJson: text("metadata_json").notNull().default("{}"),
    ipHash: text("ip_hash"),
    createdAt: text("created_at").notNull(),
  },
  (t) => [
    index("idx_audit_logs_tenant").on(t.tenantId, t.createdAt),
    index("idx_audit_logs_action").on(t.action, t.createdAt),
  ],
);

/* ------------------------------------------------------------------ */
/* InferSelect / InferInsert type exports                             */
/* ------------------------------------------------------------------ */
export type Tenant = typeof tenants.$inferSelect;
export type NewTenant = typeof tenants.$inferInsert;
export type User = typeof users.$inferSelect;
export type NewUser = typeof users.$inferInsert;
export type Workspace = typeof workspaces.$inferSelect;
export type NewWorkspace = typeof workspaces.$inferInsert;
export type AgentRun = typeof agentRuns.$inferSelect;
export type NewAgentRun = typeof agentRuns.$inferInsert;
export type AgentStep = typeof agentSteps.$inferSelect;
export type NewAgentStep = typeof agentSteps.$inferInsert;
export type SecurityScan = typeof securityScans.$inferSelect;
export type NewSecurityScan = typeof securityScans.$inferInsert;
export type SecurityFinding = typeof securityFindings.$inferSelect;
export type NewSecurityFinding = typeof securityFindings.$inferInsert;
export type Job = typeof jobs.$inferSelect;
export type NewJob = typeof jobs.$inferInsert;
export type UsageEvent = typeof usageEvents.$inferSelect;
export type NewUsageEvent = typeof usageEvents.$inferInsert;
export type WebhookEndpoint = typeof webhookEndpoints.$inferSelect;
export type NewWebhookEndpoint = typeof webhookEndpoints.$inferInsert;
export type AuditLog = typeof auditLogs.$inferSelect;
export type NewAuditLog = typeof auditLogs.$inferInsert;
