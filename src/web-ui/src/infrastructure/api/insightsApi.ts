import { invoke } from '@tauri-apps/api/core';

// ============ Types (strict 1:1 mirror of Rust types) ============

export interface DateRange {
  start: string;
  end: string;
}

export interface AtAGlance {
  whats_working: string;
  whats_hindering: string;
  quick_wins: string;
  looking_ahead: string;
}

export interface InteractionStyle {
  narrative: string;
  key_patterns: string[];
}

export interface ProjectArea {
  name: string;
  session_count: number;
  description: string;
}

export interface BigWin {
  title: string;
  description: string;
  impact: string;
}

export interface FrictionCategory {
  category: string;
  count: number;
  description: string;
  examples: string[];
  suggestion: string;
}

export interface MdAddition {
  section: string;
  content: string;
  rationale: string;
}

export interface FeatureRecommendation {
  feature: string;
  description: string;
  example_usage: string;
  benefit: string;
}

export interface UsagePattern {
  pattern: string;
  description: string;
  detail: string;
  suggested_prompt: string;
}

export interface InsightsSuggestions {
  bitfun_md_additions: MdAddition[];
  features_to_try: FeatureRecommendation[];
  usage_patterns: UsagePattern[];
}

export interface HorizonWorkflow {
  title: string;
  whats_possible: string;
  how_to_try: string;
  copyable_prompt: string;
}

export interface FunEnding {
  headline: string;
  detail: string;
}

export interface InsightsStats {
  total_hours: number;
  msgs_per_day: number;
  top_tools: [string, number][];
  top_goals: [string, number][];
  outcomes: Record<string, number>;
  satisfaction: Record<string, number>;
  session_types: Record<string, number>;
  languages: Record<string, number>;
  hour_counts: Record<number, number>;
  agent_types: Record<string, number>;
  response_time_buckets: Record<string, number>;
  median_response_time_secs: number | null;
  avg_response_time_secs: number | null;
  friction: Record<string, number>;
  success: Record<string, number>;
  tool_errors: Record<string, number>;
  total_lines_added: number;
  total_lines_removed: number;
  total_files_modified: number;
}

export interface InsightsReport {
  generated_at: number;
  date_range: DateRange;
  total_sessions: number;
  analyzed_sessions: number;
  total_messages: number;
  days_covered: number;

  stats: InsightsStats;

  at_a_glance: AtAGlance;
  interaction_style: InteractionStyle;
  project_areas: ProjectArea[];
  big_wins: BigWin[];
  friction_categories: FrictionCategory[];
  suggestions: InsightsSuggestions;
  horizon_intro: string;
  on_the_horizon: HorizonWorkflow[];
  fun_ending: FunEnding | null;

  html_report_path: string | null;
}

export interface InsightsReportMeta {
  generated_at: number;
  total_sessions: number;
  analyzed_sessions: number;
  date_range: DateRange;
  path: string;
  total_messages: number;
  days_covered: number;
  total_hours: number;
  top_goals: string[];
  languages: string[];
}

export interface InsightsProgressEvent {
  message: string;
  stage: string;
  current: number;
  total: number;
}

// ============ API client ============

export const insightsApi = {
  async generateInsights(days?: number): Promise<InsightsReport> {
    return invoke('generate_insights', {
      request: { days: days ?? 30 },
    });
  },

  async getLatestInsights(): Promise<InsightsReportMeta[]> {
    return invoke('get_latest_insights');
  },

  async loadReport(path: string): Promise<InsightsReport> {
    return invoke('load_insights_report', {
      request: { path },
    });
  },

  async hasInsightsData(days?: number): Promise<boolean> {
    return invoke('has_insights_data', {
      request: { days: days ?? 30 },
    });
  },

  async cancelGeneration(): Promise<void> {
    return invoke('cancel_insights_generation');
  },
};
