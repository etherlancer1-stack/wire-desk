//! Multi-agent orchestration engine (specs #86-120).
//! Wires together the LLM router, tool registry, prompt registry, and repo.

use crate::config::AppConfig;
use crate::error::WireResult;
use crate::llm::LlmRouter;
use crate::models::{AgentRun, RunStatus, utc_now};
use crate::prompt::PromptRegistry;
use crate::repository::Repository;
use crate::router::{AgentRouter, AgentOutcome};
use crate::tools::ToolRegistry;
use crate::security::ScopeAllowlist;
use std::path::Path;
use std::sync::Arc;

pub struct AgentEngine {
    config: Arc<AppConfig>,
    repo: Arc<Repository>,
    llm: Arc<LlmRouter>,
    tools: Arc<ToolRegistry>,
    router: AgentRouter,
}

impl AgentEngine {
    pub fn new(config: Arc<AppConfig>, repo: Repository) -> Self {
        let repo = Arc::new(repo);
        let llm = Arc::new(LlmRouter::new(config.clone()));
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let tools = Arc::new(ToolRegistry::new(workspace_root, ScopeAllowlist::default(), config.agent_tool_output_cap_chars));
        let router = AgentRouter::new(llm.clone(), tools.clone(), repo.clone(), config.agent_max_iterations);
        Self { config, repo, llm, tools, router }
    }

    /// Execute an agent run end-to-end, persisting trace and metrics.
    pub async fn run(&self, run: &AgentRun) -> WireResult<AgentOutcome> {
        let outcome = self.router.run(&run.id, &run.workspace_id, &run.goal).await?;
        // Persist trace + cost + tokens.
        let trace_json = serde_json::to_string(&outcome.trace).unwrap_or_default();
        self.repo.update_agent_run_trace_and_cost(
            &run.id, &trace_json, outcome.tokens_in, outcome.tokens_out, outcome.cost_usd,
        )?;
        Ok(outcome)
    }

    pub fn tools(&self) -> Arc<ToolRegistry> {
        self.tools.clone()
    }

    pub fn llm(&self) -> Arc<LlmRouter> {
        self.llm.clone()
    }
}

impl Clone for AgentEngine {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            repo: Arc::clone(&self.repo),
            llm: Arc::clone(&self.llm),
            tools: Arc::clone(&self.tools),
            router: AgentRouter::new(self.llm.clone(), self.tools.clone(), self.repo.clone(), self.config.agent_max_iterations),
        }
    }
}
