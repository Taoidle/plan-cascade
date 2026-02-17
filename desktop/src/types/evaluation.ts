/**
 * Evaluation Framework Types
 *
 * TypeScript interfaces matching the Rust types in
 * desktop/src-tauri/src/services/agent_composer/eval_types.rs
 */

/** An evaluator configuration defining how to assess agent performance */
export interface Evaluator {
  /** Unique evaluator identifier */
  id: string;
  /** Human-readable name */
  name: string;
  /** Evaluation criteria configuration */
  criteria: EvaluationCriteria;
}

/** Criteria for evaluating agent performance */
export interface EvaluationCriteria {
  /** Tool trajectory comparison configuration */
  tool_trajectory?: ToolTrajectoryConfig | null;
  /** Response similarity comparison configuration */
  response_similarity?: ResponseSimilarityConfig | null;
  /** LLM judge evaluation configuration */
  llm_judge?: LlmJudgeConfig | null;
}

/** Configuration for tool trajectory evaluation */
export interface ToolTrajectoryConfig {
  /** Expected tools that should be called */
  expected_tools: string[];
  /** Whether the order of tool calls matters */
  order_matters: boolean;
}

/** Configuration for response similarity evaluation */
export interface ResponseSimilarityConfig {
  /** The reference response to compare against */
  reference_response: string;
  /** Similarity threshold (0.0-1.0) for passing */
  threshold: number;
}

/** Configuration for LLM judge evaluation */
export interface LlmJudgeConfig {
  /** The model to use as judge */
  judge_model: string;
  /** The provider for the judge model */
  judge_provider: string;
  /** Rubric describing how to evaluate the response */
  rubric: string;
}

/** A test case for evaluation */
export interface EvaluationCase {
  /** Unique case identifier */
  id: string;
  /** Human-readable case name */
  name: string;
  /** Input to provide to the agent (structured JSON) */
  input: Record<string, unknown>;
  /** Optional expected output for comparison */
  expected_output?: string | null;
  /** Optional expected tool calls */
  expected_tools?: string[] | null;
}

/** Configuration for a model to evaluate */
export interface ModelConfig {
  /** Provider name (e.g., "anthropic", "openai") */
  provider: string;
  /** Model identifier */
  model: string;
  /** Optional display name for UI */
  display_name?: string | null;
}

/** An evaluation run executing cases across multiple models */
export interface EvaluationRun {
  /** Unique run identifier */
  id: string;
  /** ID of the evaluator to use */
  evaluator_id: string;
  /** Models to evaluate */
  models: ModelConfig[];
  /** Test cases to run */
  cases: EvaluationCase[];
  /** Current status */
  status: string;
  /** When the run was created (ISO 8601) */
  created_at: string;
}

/** Summary information about an evaluation run (for list views) */
export interface EvaluationRunInfo {
  /** Run identifier */
  id: string;
  /** Evaluator ID used */
  evaluator_id: string;
  /** Number of models */
  model_count: number;
  /** Number of cases */
  case_count: number;
  /** Current status */
  status: string;
  /** When created */
  created_at: string;
}

/** Summary information about an evaluator (for list views) */
export interface EvaluatorInfo {
  /** Evaluator identifier */
  id: string;
  /** Evaluator name */
  name: string;
  /** Whether tool trajectory is enabled */
  has_tool_trajectory: boolean;
  /** Whether response similarity is enabled */
  has_response_similarity: boolean;
  /** Whether LLM judge is enabled */
  has_llm_judge: boolean;
}

/** Results from evaluating a single model */
export interface EvaluationReport {
  /** Evaluation run ID */
  run_id: string;
  /** Model that was evaluated */
  model: string;
  /** Provider used */
  provider: string;
  /** Per-case results */
  results: TestResult[];
  /** Overall score (0.0-1.0) */
  overall_score: number;
  /** Total duration in milliseconds */
  duration_ms: number;
  /** Total tokens consumed */
  total_tokens: number;
  /** Estimated cost in USD */
  estimated_cost: number;
}

/** Result for a single test case */
export interface TestResult {
  /** Case identifier */
  case_id: string;
  /** Whether the case passed */
  passed: boolean;
  /** Score for this case (0.0-1.0) */
  score: number;
  /** Details about the scoring */
  details: string;
  /** Tool calls made by the agent */
  tool_calls: string[];
  /** Agent's response text */
  response: string;
}

/** Standard Tauri command response wrapper */
export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

/** Evaluation progress event from Tauri backend */
export interface EvaluationProgressEvent {
  type: 'model_started' | 'case_started' | 'case_completed' | 'model_completed' | 'run_completed' | 'error';
  model?: string;
  provider?: string;
  case_id?: string;
  score?: number;
  message?: string;
}

/** Helper to create a default EvaluationCriteria */
export function createDefaultCriteria(): EvaluationCriteria {
  return {
    tool_trajectory: null,
    response_similarity: null,
    llm_judge: null,
  };
}

/** Helper to create a default EvaluationCase */
export function createDefaultCase(id: string): EvaluationCase {
  return {
    id,
    name: `Test Case ${id}`,
    input: { prompt: '' },
    expected_output: null,
    expected_tools: null,
  };
}

/** Helper to create a default Evaluator */
export function createDefaultEvaluator(): Evaluator {
  return {
    id: '',
    name: 'New Evaluator',
    criteria: createDefaultCriteria(),
  };
}
