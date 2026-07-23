/**
 * Smart recommendations types
 */

export interface RecommendationAction {
  id: string;
  label: string;
  /** Icon name from lucide */
  icon?: string;
  description?: string;
  /** Action type */
  type?: 'primary' | 'secondary' | 'danger';
  onClick: () => void | Promise<void>;
  disabled?: boolean;
  loading?: boolean;
}

export interface RecommendationContext {
  workspacePath?: string;
  sessionId?: string;
  turnIndex?: number;
  /** List of modified files */
  modifiedFiles?: string[];
  [key: string]: any;
}

export interface IRecommendationProvider {
  id: string;
  name: string;
  /** Higher value means higher priority */
  priority: number;

  /**
   * Check whether recommendations should be shown.
   */
  shouldShow(context: RecommendationContext): Promise<boolean>;

  /**
   * Fetch recommendation actions for the given context.
   */
  getActions(context: RecommendationContext): Promise<RecommendationAction[]>;

  /**
   * Dispose resources if needed.
   */
  dispose?(): void;
}

export type RecommendationProviderFactory = () => IRecommendationProvider;

