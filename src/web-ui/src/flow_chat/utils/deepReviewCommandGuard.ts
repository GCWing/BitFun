import {
  isReviewActivityBlocking,
  type SessionReviewActivity,
} from './sessionReviewActivity';
import { REVIEW_COMMAND_RE } from './deepReviewConstants';

export function shouldBlockReviewCommand(
  input: string,
  activity?: SessionReviewActivity | null,
): boolean {
  return REVIEW_COMMAND_RE.test(input.trim()) && isReviewActivityBlocking(activity);
}

export const shouldBlockDeepReviewCommand = shouldBlockReviewCommand;
