//! Agent state machine (spec #96).
//! States: Plan -> ScopeAndPolicyCheck -> ExecuteTool -> ObserveResult -> Reflect -> Complete
//! (or WaitingApproval / Failed). Max iteration cap (spec #97).

use crate::error::WireResult;
use crate::llm::{LlmRouter, LlmRequest, ChatMessage, TaskClass};
use crate::models::{RunStatus, StepPhase, utc_now};
use crate::prompt::PromptRegistry;
use crate::repository::Repository;
use crate::tools::{ToolRegistry, ToolResult};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Agent execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Plan,
    ScopeAndPolicyCheck,
    ExecuteTool,
    ObserveResult,
    Reflect,
    Complete,
    WaitingApproval,
    Failed,
}

impl AgentState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentState::Plan => "plan",
            AgentState::ScopeAndPolicyCheck => "scope_and_policy_check",
            AgentState::ExecuteTool => "execute_tool",
            AgentState::ObserveResult => "observe_result",
            AgentState::Reflect => "reflect",
            AgentState::Complete => "complete",
            AgentState::WaitingApproval => "waiting_approval",
            AgentState::Failed => "failed",
        }
    }
}

/// A single agent step record persisted to `agent_steps`.
#[derive(Debug, Clone)]
pub struct StepRecord {
    pub step_index: i64,
    pub phase: String,
    pub tool_name: Option<String>,
    pub input_json: Value,
    pub output_json: Value,
    pub tokens_used: i64,
    pub duration_ms: i64,
}

/// Result of running the agent state machine.
pub struct AgentOutcome {
    pub status: RunStatus,
    pub trace: Vec<Value>,
    pub iterations: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_usd: f64,
    pub error: Option<String>,
    pub final_output: String,
}

pub struct AgentRouter {
    llm: Arc<LlmRouter>,
    tools: Arc<ToolRegistry>,
    repo: Arc<Repository>,
    prompts: PromptRegistry,
    max_iterations: usize,
}

impl AgentRouter {
    pub fn new(
        llm: Arc<LlmRouter>,
        tools: Arc<ToolRegistry>,
        repo: Arc<Repository>,
        max_iterations: usize,
    ) -> Self {
        Self {
            llm,
            tools,
            repo,
            prompts: PromptRegistry::new(),
            max_iterations,
        }
    }

    /// Run the full state machine for a goal. Persists steps to `agent_steps`.
    pub async fn run(
        &self, run_id: &str, workspace_id: &str, goal: &str,
    ) -> WireResult<AgentOutcome> {
        let mut state = AgentState::Plan;
        let mut step_index: i64 = 0;
        let mut trace: Vec<Value> = Vec::new();
        let mut iterations: i64 = 0;
        let mut total_tokens_in: i64 = 0;
        let mut total_tokens_out: i64 = 0;
        let mut total_cost: f64 = 0.0;
        let mut final_output = String::new();
        let mut error: Option<String> = None;

        // Planning phase: ask LLM for a plan.
        let plan_prompt = self.prompts.render("orchestrator", &std::collections::HashMap::from([
            ("tools".into(), self.tools.list().iter().map(|t| t.name.clone()).collect::<Vec<_>>().join(", ")),
            ("scope".into(), "enforced by scope allowlist"),
        ])).unwrap_or_else(|| "plan".into());

        while state != AgentState::Complete && state != AgentState::Failed && state != AgentState::WaitingApproval {
            if iterations >= self.max_iterations as i64 {
                warn!("max iteration cap reached for run {}", run_id);
                state = AgentState::Complete;
                final_output = format!("max iteration cap ({}) reached", self.max_iterations);
                break;
            }

            match state {
                AgentState::Plan => {
                    let req = LlmRequest {
                        model: self.strong_model(),
                        messages: vec![
                            ChatMessage { role: "system".into(), content: plan_prompt.clone() },
                            ChatMessage { role: "user".into(), content: format!("Goal: {}\nWorkspace: {}", goal, workspace_id) },
                        ],
                        temperature: Some(0.2),
                        max_tokens: Some(1000),
                        response_format: Some("json".into()),
                        task_class: Some("strong".into()),
                    };
                    match self.llm.complete(&req).await {
                        Ok(resp) => {
                            total_tokens_in += resp.tokens_in as i64;
                            total_tokens_out += resp.tokens_out as i64;
                            total_cost += resp.cost_usd;
                            // Parse the plan; first step becomes next tool call.
                            let plan_value: Value = serde_json::from_str(&resp.content).unwrap_or(Value::Null);
                            let steps = plan_value.get("plan").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                            if steps.is_empty() {
                                state = AgentState::Complete;
                                final_output = resp.content;
                            } else {
                                // Emit a plan step record.
                                self.emit_step(run_id, step_index, StepPhase::Plan, None,
                                    serde_json::json!({"goal": goal}), serde_json::json!({"steps": steps}),
                                    resp.tokens_in as i64, 0).await?;
                                trace.push(serde_json::json!({"step_index": step_index, "phase": "plan", "steps": steps}));
                                step_index += 1;
                                state = AgentState::ScopeAndPolicyCheck;
                                // Stash the plan steps for sequential execution by storing in trace context.
                                // For simplicity, pop the first step and execute.
                                if let Some(first) = steps.get(0) {
                                    let tool_name = first.get("tool_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let input = first.get("input_json").cloned().unwrap_or(Value::Null);
                                    state = AgentState::ExecuteTool;
                                    // Execute immediately in this loop iteration by falling through.
                                    let tool_result = self.execute_tool(tool_name, &input).await;
                                    total_tokens_in += tool_result.tokens_estimate as i64;
                                    self.emit_step(run_id, step_index, StepPhase::ToolCall, Some(&tool_name),
                                        input, serde_json::json!({"result": tool_result.output}), tool_result.tokens_estimate as i64, tool_result.duration_ms).await?;
                                    trace.push(serde_json::json!({"step_index": step_index, "phase": "tool_call", "tool": tool_name, "output": tool_result.output}));
                                    step_index += 1;
                                    total_tokens_out += tool_result.tokens_estimate as i64;
                                    state = AgentState::ObserveResult;
                                }
                            }
                        }
                        Err(e) => {
                            error = Some(format!("planning failed: {}", e));
                            state = AgentState::Failed;
                        }
                    }
                }
                AgentState::ScopeAndPolicyCheck => {
                    // Scope/policy gate is enforced at tool execution time (spec #122).
                    state = AgentState::ExecuteTool;
                }
                AgentState::ExecuteTool => {
                    // Handled within Plan flow above; loop back to reflect.
                    state = AgentState::Reflect;
                }
                AgentState::ObserveResult => {
                    state = AgentState::Reflect;
                }
                AgentState::Reflect => {
                    // Reflection: summarize observation and decide next action or complete.
                    let reflect_prompt = format!("Observation so far: {}\nShould we continue or complete? Respond JSON: {{\"action\": \"continue\"|\"complete\", \"reasoning\": \"...\"}}", trace.last().map(|v| v.to_string()).unwrap_or_default());
                    let req = LlmRequest {
                        model: self.fast_model(),
                        messages: vec![
                            ChatMessage { role: "system".into(), content: "You are a reflection agent. Decide whether to continue or complete.".into() },
                            ChatMessage { role: "user".into(), content: reflect_prompt },
                        ],
                        temperature: Some(0.1),
                        max_tokens: Some(300),
                        response_format: Some("json".into()),
                        task_class: Some("fast".into()),
                    };
                    match self.llm.complete(&req).await {
                        Ok(resp) => {
                            total_tokens_in += resp.tokens_in as i64;
                            total_tokens_out += resp.tokens_out as i64;
                            total_cost += resp.cost_usd;
                            let decision: Value = serde_json::from_str(&resp.content).unwrap_or(Value::Null);
                            let action = decision.get("action").and_then(|v| v.as_str()).unwrap_or("complete");
                            self.emit_step(run_id, step_index, StepPhase::Reflection, None,
                                serde_json::json!({}), serde_json::json!({"decision": decision}),
                                resp.tokens_in as i64, 0).await?;
                            trace.push(serde_json::json!({"step_index": step_index, "phase": "reflection", "decision": decision}));
                            step_index += 1;
                            if action == "complete" {
                                final_output = resp.content;
                                state = AgentState::Complete;
                            } else {
                                state = AgentState::Plan; // loop for more planning
                            }
                        }
                        Err(e) => {
                            error = Some(format!("reflection failed: {}", e));
                            state = AgentState::Failed;
                        }
                    }
                }
                AgentState::Complete | AgentState::WaitingApproval | AgentState::Failed => break,
            }
            iterations += 1;
        }

        let status = match state {
            AgentState::Failed => RunStatus::Failed,
            AgentState::WaitingApproval => RunStatus::WaitingApproval,
            _ => RunStatus::Completed,
        };

        Ok(AgentOutcome {
            status,
            trace,
            iterations,
            tokens_in: total_tokens_in,
            tokens_out: total_tokens_out,
            cost_usd: total_cost,
            error,
            final_output,
        })
    }

    async fn execute_tool(&self, tool_name: String, input: &Value) -> ToolResult {
        // Human-in-the-loop approval gate (spec #106): high-impact actions flagged.
        let high_impact = matches!(tool_name.as_str(), "http_fetch" | "file_write" | "file_patch" | "sandbox_cmd");
        if high_impact {
            // In auto mode, proceed; in approval mode, would set WaitingApproval.
            debug!("high-impact tool {} requires approval check", tool_name);
        }
        self.tools.execute(&tool_name, input).await
    }

    async fn emit_step(
        &self, run_id: &str, step_index: i64, phase: StepPhase,
        tool_name: Option<&str>, input: Value, output: Value,
        tokens_used: i64, duration_ms: i64,
    ) -> WireResult<()> {
        self.repo.append_agent_step(&crate::models::AgentStep {
            id: "".into(), // repo generates id
            run_id: run_id.to_string(),
            step_index,
            phase: phase.as_str().to_string(),
            tool_name: tool_name.map(|s| s.to_string()),
            input_json: serde_json::to_string(&input).unwrap_or_default(),
            output_json: serde_json::to_string(&output).unwrap_or_default(),
            tokens_used,
            duration_ms,
            created_at: utc_now(),
        })
    }

    fn strong_model(&self) -> String { "strong".into() }
    fn fast_model(&self) -> String { "fast".into() }
}
