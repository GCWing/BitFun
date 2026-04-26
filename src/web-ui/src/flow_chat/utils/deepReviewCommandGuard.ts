import {
  isReviewActivityBlocking,
  type SessionReviewActivity,
} from './sessionReviewActivity';
import { DEEP_REVIEW_COMMAND_RE } from './deepReviewConstants';

export function shouldBlockDeepReviewCommand(
  input: string,
  activity?: SessionReviewActivity | null,
): boolean {
  return DEEP_REVIEW_COMMAND_RE.test(input.trim()) && isReviewActivityBlocking(activity);
}
