/**
 * 18 built-in orchestration patterns for legion templates.
 * Each pattern maps to the orchestration-patterns skill library.
 */
export interface LegionPatternNode {
  id: string;
  agent: string;
  role: string;
  prompt: string;
  gate?: boolean;
}

export interface LegionPatternEdge {
  from: string;
  to: string;
  condition?: string;
}

export interface LegionPattern {
  id: string;
  name: string;
  description: string;
  complexityLevel: number;
  nodes: LegionPatternNode[];
  edges: LegionPatternEdge[];
}

const PATTERNS: LegionPattern[] = [
  {
    id: 'sparc-dev',
    name: 'SPARC Development',
    description: '5-stage SPARC development pipeline: specification → pseudocode → architecture → refinement → completion',
    complexityLevel: 4,
    nodes: [
      { id: 'researcher',   agent: 'Plan',    role: 'Research Bee',     prompt: 'Gather requirements, define acceptance criteria, identify constraints and edge cases.' },
      { id: 'decomposer',   agent: 'Plan',    role: 'Decompose Bee',    prompt: 'Decompose into executable sub-tasks, annotate complexity, define dependencies.' },
      { id: 'architect',    agent: 'agentic', role: 'Architect Bee',    prompt: 'Design modules, define interfaces, resolve constraints.' },
      { id: 'implementer',  agent: 'agentic', role: 'Implement Bee',    prompt: 'Implement according to architecture and interface contracts.' },
      { id: 'tester',       agent: 'agentic', role: 'Test Bee',         prompt: 'Write and run automated tests. Coverage ≥ 80%, all ACs pass.' },
      { id: 'reviewer',     agent: 'DeepReview', role: 'Review Bee',    prompt: 'Code review and documentation generation.', gate: true },
    ],
    edges: [
      { from: 'researcher',  to: 'decomposer' },
      { from: 'decomposer',  to: 'architect' },
      { from: 'architect',   to: 'implementer' },
      { from: 'architect',   to: 'tester' },
      { from: 'implementer', to: 'reviewer' },
      { from: 'tester',      to: 'reviewer' },
      { from: 'reviewer',    to: 'implementer', condition: 'fail' },
      { from: 'reviewer',    to: 'tester',      condition: 'fail' },
    ],
  },
  {
    id: 'cicd-pipeline',
    name: 'CI/CD Pipeline',
    description: 'Lint → Unit test → Build → Integration test → Security audit → Deploy → Verify',
    complexityLevel: 5,
    nodes: [
      { id: 'lint',           agent: 'agentic', role: 'Lint Bee',        prompt: 'Run linter, type checker, security scan. Gate: zero errors.' },
      { id: 'unit-test',      agent: 'agentic', role: 'Unit Test Bee',   prompt: 'Run unit tests across multiple environments. Gate: all pass, coverage ≥ 80%.' },
      { id: 'build',          agent: 'agentic', role: 'Build Bee',       prompt: 'Compile, package, upload artifact. Gate: build succeeds.' },
      { id: 'integration',    agent: 'agentic', role: 'Integration Bee', prompt: 'Deploy to staging, run integration tests, smoke test.' },
      { id: 'security-audit', agent: 'agentic', role: 'Security Bee',    prompt: 'Dependency vulnerability scan, container scan, compliance check.' },
      { id: 'deploy',         agent: 'agentic', role: 'Deploy Bee',      prompt: 'Rollout with health check. Gate: health passes.' },
      { id: 'verify',         agent: 'DeepReview', role: 'Verify Bee',   prompt: 'Smoke test production, monitor metrics, rollback if needed.', gate: true },
    ],
    edges: [
      { from: 'lint',           to: 'unit-test' },
      { from: 'unit-test',      to: 'build' },
      { from: 'build',          to: 'integration' },
      { from: 'integration',    to: 'security-audit' },
      { from: 'security-audit', to: 'deploy' },
      { from: 'deploy',         to: 'verify' },
    ],
  },
  {
    id: 'fan-out-converge',
    name: 'Fan-out Converge',
    description: 'Dispatch → Parallel research (N bees) → Synthesize → Final review',
    complexityLevel: 5,
    nodes: [
      { id: 'dispatch',     agent: 'Team',    role: 'Commander',       prompt: 'Evaluate task, match pattern, build team, assign sub-goals.' },
      { id: 'researcher-1', agent: 'agentic', role: 'Research Bee A',  prompt: 'Research scope A independently and report structured results.' },
      { id: 'researcher-2', agent: 'agentic', role: 'Research Bee B',  prompt: 'Research scope B independently and report structured results.' },
      { id: 'researcher-3', agent: 'agentic', role: 'Research Bee C',  prompt: 'Research scope C independently and report structured results.' },
      { id: 'synthesizer',  agent: 'agentic', role: 'Synthesize Bee',  prompt: 'Collect results, resolve conflicts, merge outputs, check consistency.' },
      { id: 'reviewer',     agent: 'DeepReview', role: 'Review Bee',   prompt: 'Review merged output, generate final report.', gate: true },
    ],
    edges: [
      { from: 'dispatch',     to: 'researcher-1' },
      { from: 'dispatch',     to: 'researcher-2' },
      { from: 'dispatch',     to: 'researcher-3' },
      { from: 'researcher-1', to: 'synthesizer' },
      { from: 'researcher-2', to: 'synthesizer' },
      { from: 'researcher-3', to: 'synthesizer' },
      { from: 'synthesizer',  to: 'reviewer' },
      { from: 'reviewer',     to: 'synthesizer', condition: 'fail' },
    ],
  },
  {
    id: 'triad-minimal',
    name: 'Three-Bee Minimal',
    description: 'Prompt Bee → Execute Bee → Review Bee. Atomic execution unit.',
    complexityLevel: 2,
    nodes: [
      { id: 'prompt-bee',  agent: 'Plan',     role: 'Prompt Bee',   prompt: 'Analyze task, inject relevant skills and templates.' },
      { id: 'execute-bee', agent: 'agentic',  role: 'Execute Bee',  prompt: 'Execute the task using the provided methodology.' },
      { id: 'review-bee',  agent: 'DeepReview', role: 'Review Bee', prompt: 'Audit behavior and output, gate pass/fail.', gate: true },
    ],
    edges: [
      { from: 'prompt-bee',  to: 'execute-bee' },
      { from: 'execute-bee', to: 'review-bee' },
      { from: 'review-bee',  to: 'execute-bee', condition: 'fail' },
      { from: 'review-bee',  to: 'prompt-bee',  condition: 'fail' },
    ],
  },
  {
    id: 'state-machine',
    name: 'State Machine',
    description: 'Multi-state flow with conditional branches and escalation.',
    complexityLevel: 6,
    nodes: [
      { id: 'pending',    agent: 'Plan',     role: 'Assess Bee',   prompt: 'Evaluate task complexity and route to appropriate state.' },
      { id: 'executing',  agent: 'agentic',  role: 'Execute Bee',  prompt: 'Execute. On success → review. On failure (≤3) → retry. On failure (>3) → escalate.' },
      { id: 'reviewing',  agent: 'DeepReview', role: 'Review Bee', prompt: 'Review. On pass → complete. On fix (≤3 rounds) → back to executing.' },
      { id: 'escalated',  agent: 'Team',     role: 'Escalation',   prompt: 'Human-in-the-loop decision: confirm fix or abandon.' },
      { id: 'completed',  agent: 'agentic',  role: 'Doc Bee',      prompt: 'Generate completion report.' },
      { id: 'failed',     agent: 'agentic',  role: 'Doc Bee',      prompt: 'Generate failure report with root cause.' },
    ],
    edges: [
      { from: 'pending',   to: 'executing' },
      { from: 'executing', to: 'reviewing',  condition: 'success' },
      { from: 'executing', to: 'failed',     condition: 'exhausted' },
      { from: 'reviewing', to: 'completed',  condition: 'pass' },
      { from: 'reviewing', to: 'executing',  condition: 'fix' },
      { from: 'reviewing', to: 'escalated',  condition: 'max_rounds' },
      { from: 'escalated', to: 'executing',  condition: 'confirm' },
      { from: 'escalated', to: 'failed',     condition: 'abandon' },
    ],
  },
  {
    id: 'deep-research',
    name: 'Deep Research',
    description: '6-phase research pipeline with parallel specialists, debate, and arbitration.',
    complexityLevel: 6,
    nodes: [
      { id: 'planner',      agent: 'Plan',          role: 'Planner',          prompt: 'Query understanding, ambiguity detection, sub-question decomposition.' },
      { id: 'primary',      agent: 'agentic',       role: 'Primary Source',   prompt: 'Primary source specialist research.' },
      { id: 'news',         agent: 'agentic',       role: 'News Specialist',  prompt: 'News and timeline research.' },
      { id: 'expert',       agent: 'agentic',       role: 'Expert Opinion',   prompt: 'Expert opinion research.' },
      { id: 'counter',      agent: 'agentic',       role: 'Counter Evidence', prompt: 'Counter-evidence research.' },
      { id: 'advocate',     agent: 'agentic',       role: 'Advocate',         prompt: 'Defend findings in adversarial debate.' },
      { id: 'critic',       agent: 'agentic',       role: 'Critic',           prompt: 'Challenge findings in adversarial debate.' },
      { id: 'fact-checker', agent: 'agentic',       role: 'Fact Checker',     prompt: 'Resolve conflicts into HARD_CONFLICT / GENUINE_UNCERTAINTY / UNVERIFIED.' },
      { id: 'arbitrator',   agent: 'DeepReview',    role: 'Arbitrator',       prompt: 'Research Manager arbitration with verdict markers.', gate: true },
      { id: 'reporter',     agent: 'agentic',       role: 'Reporter',         prompt: 'Generate final report with citation index.' },
    ],
    edges: [
      { from: 'planner',      to: 'primary' },
      { from: 'planner',      to: 'news' },
      { from: 'planner',      to: 'expert' },
      { from: 'planner',      to: 'counter' },
      { from: 'primary',      to: 'advocate' },
      { from: 'news',         to: 'advocate' },
      { from: 'expert',       to: 'advocate' },
      { from: 'counter',      to: 'critic' },
      { from: 'advocate',     to: 'fact-checker' },
      { from: 'critic',       to: 'fact-checker' },
      { from: 'fact-checker', to: 'arbitrator' },
      { from: 'arbitrator',   to: 'reporter',  condition: 'pass' },
      { from: 'arbitrator',   to: 'fact-checker', condition: 'contest' },
    ],
  },
  {
    id: 'react-loop',
    name: 'ReAct Loop',
    description: 'Thought → Action → Observation loop with stop condition.',
    complexityLevel: 1,
    nodes: [
      { id: 'react-agent', agent: 'agentic', role: 'ReAct Agent', prompt: 'Think → Act → Observe loop until stop condition or final answer.' },
    ],
    edges: [],
  },
  {
    id: 'plan-exec-reflect',
    name: 'Plan-Execute-Reflect',
    description: 'Plan → Execute step by step → Draft → Reflect and critique → Refine or stop.',
    complexityLevel: 3,
    nodes: [
      { id: 'planner',    agent: 'Plan',    role: 'Planner',    prompt: 'Create a structured plan with dependencies.' },
      { id: 'executor',   agent: 'agentic', role: 'Executor',   prompt: 'Execute the plan step by step.' },
      { id: 'reflector',  agent: 'DeepReview', role: 'Reflector', prompt: 'Reflect on the draft, critique quality, decide refine or stop.', gate: true },
    ],
    edges: [
      { from: 'planner',   to: 'executor' },
      { from: 'executor',  to: 'reflector' },
      { from: 'reflector', to: 'executor', condition: 'refine' },
    ],
  },
  {
    id: 'event-driven',
    name: 'Event-Driven Response',
    description: 'Detect → Classify → Triage → Resolve → Postmortem. For incidents and alerts.',
    complexityLevel: 4,
    nodes: [
      { id: 'detector',    agent: 'agentic', role: 'Detector',    prompt: 'Detect event source, classify severity (P0-P4), tag category.' },
      { id: 'triage',     agent: 'agentic', role: 'Triage',      prompt: 'Assess impact, identify root cause, propose fix.' },
      { id: 'resolver',   agent: 'agentic', role: 'Resolver',    prompt: 'Apply fix, verify resolution, restore service.' },
      { id: 'postmortem', agent: 'agentic', role: 'Postmortem',  prompt: 'Document timeline, identify prevention, generate report.' },
    ],
    edges: [
      { from: 'detector',  to: 'triage' },
      { from: 'triage',    to: 'resolver' },
      { from: 'resolver',  to: 'postmortem' },
    ],
  },
  {
    id: 'coding-agent',
    name: 'Coding Agent',
    description: 'Repo inspection → Scoped plan → File edits → Tests & checks → Patch & summary.',
    complexityLevel: 3,
    nodes: [
      { id: 'inspector',  agent: 'agentic', role: 'Inspector',  prompt: 'Inspect repository structure, understand codebase.' },
      { id: 'planner',    agent: 'Plan',    role: 'Planner',    prompt: 'Create scoped implementation plan.' },
      { id: 'editor',     agent: 'agentic', role: 'Editor',     prompt: 'Implement changes with minimal diff.' },
      { id: 'tester',     agent: 'agentic', role: 'Tester',     prompt: 'Run tests and checks.' },
      { id: 'reviewer',   agent: 'DeepReview', role: 'Reviewer', prompt: 'Review diff, logs, summary.', gate: true },
    ],
    edges: [
      { from: 'inspector', to: 'planner' },
      { from: 'planner',   to: 'editor' },
      { from: 'editor',    to: 'tester' },
      { from: 'tester',    to: 'reviewer' },
      { from: 'reviewer',  to: 'editor', condition: 'fail' },
    ],
  },
  {
    id: 'dag-data-pipeline',
    name: 'DAG Data Pipeline',
    description: 'Extract → Transform (parallel partitions) → Validate → Load → Report.',
    complexityLevel: 4,
    nodes: [
      { id: 'extract',    agent: 'agentic', role: 'Extractor',   prompt: 'Connect source, validate connection, pull incremental data.' },
      { id: 'transform-a', agent: 'agentic', role: 'Transform A', prompt: 'Clean and transform partition A.' },
      { id: 'transform-b', agent: 'agentic', role: 'Transform B', prompt: 'Clean and transform partition B.' },
      { id: 'validator',  agent: 'agentic', role: 'Validator',   prompt: 'Run quality rules, check anomalies, generate quality report.' },
      { id: 'loader',     agent: 'agentic', role: 'Loader',      prompt: 'Connect target, write data, verify row count.' },
      { id: 'reporter',   agent: 'agentic', role: 'Reporter',    prompt: 'Generate execution report, log metrics.' },
    ],
    edges: [
      { from: 'extract',     to: 'transform-a' },
      { from: 'extract',     to: 'transform-b' },
      { from: 'transform-a', to: 'validator' },
      { from: 'transform-b', to: 'validator' },
      { from: 'validator',   to: 'loader' },
      { from: 'loader',      to: 'reporter' },
    ],
  },
  {
    id: 'pr-code-review',
    name: 'PR Code Review',
    description: 'PR created → Lint → Code review (max 3 rounds) → Merge → Deploy.',
    complexityLevel: 3,
    nodes: [
      { id: 'lint',        agent: 'agentic',    role: 'Lint Bee',    prompt: 'Check diff size, run automated lint, verify PR template.' },
      { id: 'reviewer',    agent: 'DeepReview', role: 'Review Bee',  prompt: 'Review logic, check test coverage, verify no regression.' },
      { id: 'merger',      agent: 'agentic',    role: 'Merge Bee',   prompt: 'Rebase, resolve conflicts, run CI again.' },
      { id: 'deployer',    agent: 'agentic',    role: 'Deploy Bee',  prompt: 'Deploy with promotion staging → production.' },
    ],
    edges: [
      { from: 'lint',     to: 'reviewer' },
      { from: 'reviewer', to: 'merger',   condition: 'approved' },
      { from: 'reviewer', to: 'lint',     condition: 'changes_requested' },
      { from: 'merger',   to: 'deployer' },
    ],
  },
  {
    id: 'deploy-orchestration',
    name: 'Deploy Orchestration',
    description: 'Configure → Schedule → Health check → Rolling update → Self-heal loop.',
    complexityLevel: 5,
    nodes: [
      { id: 'configure',    agent: 'agentic', role: 'Config Bee',    prompt: 'Define desired state, set resource limits, configure probes.' },
      { id: 'scheduler',    agent: 'agentic', role: 'Schedule Bee',  prompt: 'Match nodes, pull images, start containers.' },
      { id: 'health-check', agent: 'agentic', role: 'Health Bee',    prompt: 'Readiness, liveness, startup probes.' },
      { id: 'updater',      agent: 'agentic', role: 'Update Bee',    prompt: 'Rolling update, verify each batch, zero downtime.' },
      { id: 'healer',       agent: 'agentic', role: 'Healer Bee',    prompt: 'Continuous pod/node health monitoring, auto-restart/scale/migrate.' },
    ],
    edges: [
      { from: 'configure',    to: 'scheduler' },
      { from: 'scheduler',    to: 'health-check' },
      { from: 'health-check', to: 'updater' },
      { from: 'updater',      to: 'healer' },
    ],
  },
  {
    id: 'six-layer-runtime',
    name: 'Six-Layer Agent Runtime',
    description: 'Intent dispatch → State & memory → Execution sandbox → Tool boundary → Control → Endpoint.',
    complexityLevel: 7,
    nodes: [
      { id: 'intent',     agent: 'Team',    role: 'Intent Layer',    prompt: 'Receive task/event, dispatch to appropriate handler, spawn sub-agents.' },
      { id: 'state',      agent: 'agentic', role: 'State Layer',     prompt: 'Manage working memory, persist artifacts, create checkpoints.' },
      { id: 'exec',       agent: 'agentic', role: 'Exec Layer',      prompt: 'Execute in sandbox/container with appropriate environment.' },
      { id: 'tool',       agent: 'agentic', role: 'Tool Layer',      prompt: 'Bridge to MCP/A2A/ANP protocols, call external tools.' },
      { id: 'control',    agent: 'DeepReview', role: 'Control Layer', prompt: 'Policy approval, behavior evaluation, guard enforcement.' },
      { id: 'endpoint',   agent: 'agentic', role: 'Endpoint Layer',  prompt: 'Deliver results to user interface or API consumer.' },
    ],
    edges: [
      { from: 'intent',  to: 'state' },
      { from: 'state',   to: 'exec' },
      { from: 'exec',    to: 'tool' },
      { from: 'tool',    to: 'control' },
      { from: 'control', to: 'endpoint' },
    ],
  },
  {
    id: 'memory-retrieval',
    name: 'Memory & Retrieval',
    description: 'Working memory → Promote/discard → Episodic/Semantic memory → Retrieval → Notes → Task context.',
    complexityLevel: 4,
    nodes: [
      { id: 'working',   agent: 'agentic', role: 'Working Memory',  prompt: 'Current session state, lightweight, in-process.' },
      { id: 'episodic',  agent: 'agentic', role: 'Episodic Store',  prompt: 'Store bounded events with structured metadata + similarity search.' },
      { id: 'semantic',  agent: 'agentic', role: 'Semantic Store',  prompt: 'Persist cross-task facts, dedup, normalize relations.' },
      { id: 'retrieval', agent: 'agentic', role: 'Retrieval Layer', prompt: 'Hybrid search: keyword + dense retrieval + structured filters.' },
      { id: 'context',   agent: 'agentic', role: 'Context Builder', prompt: 'Assemble notes and artifacts into task context for model call.' },
    ],
    edges: [
      { from: 'working',   to: 'episodic' },
      { from: 'working',   to: 'semantic' },
      { from: 'episodic',  to: 'retrieval' },
      { from: 'semantic',  to: 'retrieval' },
      { from: 'retrieval', to: 'context' },
    ],
  },
  {
    id: 'customer-support',
    name: 'Customer Support',
    description: 'Triage → Policy grounding → Draft → Guardrails → Human review queue.',
    complexityLevel: 3,
    nodes: [
      { id: 'triage',    agent: 'agentic', role: 'Triage',      prompt: 'Classify case type, urgency, sentiment, requested outcome.' },
      { id: 'policy',    agent: 'agentic', role: 'Policy Agent', prompt: 'Ground response in explicit policy documents.' },
      { id: 'drafter',   agent: 'agentic', role: 'Drafter',     prompt: 'Draft response. Never auto-send — final decision is human.' },
      { id: 'guard',     agent: 'DeepReview', role: 'Guardrail', prompt: 'Reject refunds, legal commitments, high-risk actions.', gate: true },
    ],
    edges: [
      { from: 'triage',  to: 'policy' },
      { from: 'policy',  to: 'drafter' },
      { from: 'drafter', to: 'guard' },
      { from: 'guard',   to: 'drafter', condition: 'fail' },
    ],
  },
  {
    id: 'evaluation-observability',
    name: 'Evaluation & Observability',
    description: 'Offline eval → Online monitoring → Structured traces → Failure triage.',
    complexityLevel: 5,
    nodes: [
      { id: 'offline',   agent: 'agentic', role: 'Offline Eval',  prompt: 'Run benchmarks on known tasks, compare prompts/models/tools.' },
      { id: 'online',    agent: 'agentic', role: 'Online Monitor', prompt: 'Collect production signals: success rate, latency, escalation rate.' },
      { id: 'tracer',    agent: 'agentic', role: 'Tracer',        prompt: 'Capture structured traces: tool inputs/outputs, state transitions.' },
      { id: 'triage',    agent: 'DeepReview', role: 'Triage',     prompt: 'Failure triage from traces: prompt / tool / model decisions.' },
    ],
    edges: [
      { from: 'offline', to: 'triage' },
      { from: 'online',  to: 'triage' },
      { from: 'tracer',  to: 'triage' },
    ],
  },
  {
    id: 'workflow-agent-hybrid',
    name: 'Workflow-Agent Hybrid',
    description: 'Known path → workflow. Unknown path → agent. Hybrid embeds agent nodes in workflow or vice versa.',
    complexityLevel: 5,
    nodes: [
      { id: 'classifier', agent: 'Plan',    role: 'Classifier',   prompt: 'Evaluate: is the path known and rules stable (workflow) or unknown/variable (agent)?' },
      { id: 'workflow',   agent: 'agentic', role: 'Workflow',     prompt: 'Predefined ordered execution for deterministic business logic.' },
      { id: 'agent-node', agent: 'agentic', role: 'Agent Node',   prompt: 'Autonomous decision-making for bounded exploration and judgment.' },
      { id: 'compliance', agent: 'DeepReview', role: 'Compliance', prompt: 'Wrap agent outputs in workflow controls: compliance, approval, irreversible ops.', gate: true },
    ],
    edges: [
      { from: 'classifier', to: 'workflow' },
      { from: 'classifier', to: 'agent-node' },
      { from: 'agent-node', to: 'compliance' },
      { from: 'workflow',   to: 'compliance' },
    ],
  },
];

export default PATTERNS;
