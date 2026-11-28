/**
 * Wire Desk — typed API client (fetch-based, no framework).
 * Implements the consistent API envelope (spec #194), request ID propagation
 * (spec #196), rate-limit headers (spec #193), and Bearer auth.
 */

import type { ApiEnvelope } from "../types";

const BASE = (() => {
  if (typeof window !== "undefined") return window.location.origin;
  return process.env.WIREDESK_API_BASE ?? "http://localhost:8080";
})();

export class WireDeskClient {
  private base: string;
  private token?: string;

  constructor(base = BASE, token?: string) {
    this.base = base.replace(/\/$/, "");
    this.token = token;
  }

  setToken(token: string) { this.token = token; }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    opts: { idempotencyKey?: string } = {}
  ): Promise<ApiEnvelope<T>> {
    const url = `${this.base}${path}`;
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      "X-Request-Id": opts.idempotencyKey || crypto.randomUUID(),
    };
    if (this.token) headers["Authorization"] = `Bearer ${this.token}`;

    const res = await fetch(url, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
      signal: AbortSignal.timeout(30000),
    });

    const rid = res.headers.get("X-Request-Id") || headers["X-Request-Id"];
    let data: unknown = null;
    const ct = res.headers.get("content-type") ?? "";
    if (ct.includes("application/json")) {
      data = await res.json().catch(() => null);
    }

    const envelope: ApiEnvelope<T> = {
      ok: res.ok,
      data: res.ok ? (data as T) : undefined,
      error: res.ok
        ? undefined
        : {
            code: (data as any)?.error?.code ?? "INTERNAL_ERROR",
            message: (data as any)?.error?.message ?? res.statusText,
            details: (data as any)?.error?.details,
          },
      request_id: rid,
    };

    if (!res.ok) {
      const err: any = new Error(envelope.error?.message ?? `HTTP ${res.status}`);
      err.status = res.status;
      err.envelope = envelope;
      throw err;
    }
    return envelope;
  }

  /* ---------- Auth ---------- */
  async register(body: { email: string; password: string; tenant_name?: string; tenant_slug?: string }) {
    return this.request<any>("POST", "/api/v1/auth/register", body);
  }
  async login(body: { email: string; password: string }) {
    return this.request<{ token: string; user_id: string; tenant_id: string; role: string }>("POST", "/api/v1/auth/login", body);
  }
  async me() {
    return this.request<any>("GET", "/api/v1/auth/me");
  }

  /* ---------- API Keys ---------- */
  async createKey() { return this.request<any>("POST", "/api/v1/keys"); }
  async listKeys() { return this.request<any[]>("GET", "/api/v1/keys"); }
  async deleteKey(id: string) { return this.request<any>("DELETE", `/api/v1/keys/${id}`); }

  /* ---------- Workspaces ---------- */
  async createWorkspace(body: { name: string; description?: string; scope_allowlist?: any; rate_limit_rps?: number }) {
    return this.request<any>("POST", "/api/v1/workspaces", body);
  }
  async listWorkspaces() { return this.request<any[]>("GET", "/api/v1/workspaces"); }
  async getWorkspace(id: string) { return this.request<any>(`GET`, `/api/v1/workspaces/${id}`); }
  async updateScope(id: string, body: any) { return this.request<any>("PATCH", `/api/v1/workspaces/${id}/scope`, body); }

  /* ---------- Agent runs ---------- */
  async createRun(body: { goal: string; workspace_id: string; idempotency_key?: string }) {
    return this.request<any>("POST", "/api/v1/agents/runs", body, { idempotencyKey: body.idempotency_key });
  }
  async listRuns() { return this.request<any[]>("GET", "/api/v1/agents/runs"); }
  async getRun(id: string) { return this.request<any>(`GET`, `/api/v1/agents/runs/${id}`); }
  async cancelRun(id: string) { return this.request<any>("POST", `/api/v1/agents/runs/${id}/cancel`); }
  async streamRun(id: string) {
    const res = await fetch(`${this.base}/api/v1/agents/runs/${id}/stream`, {
      headers: this.token ? { Authorization: `Bearer ${this.token}` } : {},
    });
    return res;
  }

  /* ---------- Security scans ---------- */
  async createScan(body: { target: string; workspace_id: string; scan_type: string }) {
    return this.request<any>("POST", "/api/v1/scans", body);
  }
  async listScans() { return this.request<any[]>("GET", "/api/v1/scans"); }
  async getScan(id: string) { return this.request<any>(`GET`, `/api/v1/scans/${id}`); }
  async listFindings(scanId: string) { return this.request<any[]>(`GET`, `/api/v1/scans/${scanId}/findings`); }
  async updateFinding(id: string, body: { status: string }) {
    return this.request<any>("PATCH", `/api/v1/findings/${id}`, body);
  }
  async getScanReport(id: string) { return this.request<any>(`GET`, `/api/v1/scans/${id}/report`); }

  /* ---------- Billing ---------- */
  async plans() { return this.request<any[]>("GET", "/api/v1/billing/plans"); }
  async usage() { return this.request<any>("GET", "/api/v1/billing/usage"); }
  async checkout(body: { plan_id: string; tenant_id: string }) {
    return this.request<{ checkout_url: string; session_id: string }>("POST", "/api/v1/billing/checkout", body);
  }
  async portal(body: { customer_id?: string }) {
    return this.request<{ portal_url: string }>("POST", "/api/v1/billing/portal", body);
  }

  /* ---------- Webhooks / admin ---------- */
  async createWebhook(body: { url: string; events: string[] }) {
    return this.request<any>("POST", "/api/v1/webhooks", body);
  }
  async listWebhooks() { return this.request<any[]>("GET", "/api/v1/webhooks"); }
  async listAuditLogs() { return this.request<any[]>("GET", "/api/v1/audit-logs"); }
  async adminOverview() { return this.request<any>("GET", "/api/v1/admin/overview"); }
  async exportTenant() { return this.request<any>("GET", "/api/v1/export"); }

  /* ---------- Public grader ---------- */
  async publicSecurityGrade(body: { target: string }) {
    return this.request<any>("POST", "/api/v1/public/security-grade", body);
  }
}

export const wireDesk = new WireDeskClient();
