/**
 * Renders a `user-steering` flow item as a normal user message in the
 * conversation flow. The backend confirmation still updates this item by
 * `steeringId`, but the user-facing surface is intentionally identical to a
 * message sent from the composer.
 *
 * The item is appended to the *current* model round's items, so it visually
 * sits after whatever thinking / text / tool-call content has already
 * streamed. When the backend finishes the current atomic step and starts a
 * new model round, that next round renders below it — matching the user's
 * mental model of "the agent reads my steering and responds in a new turn".
 */

import { UserMessage } from './UserMessage';
import type { FlowUserSteeringItem } from '../types/flow-chat';

interface UserSteeringBubbleProps {
  item: FlowUserSteeringItem;
}

export function UserSteeringBubble({ item }: UserSteeringBubbleProps): JSX.Element {
  return (
    <UserMessage
      message={item.content}
      timestamp={item.timestamp}
    />
  );
}

export default UserSteeringBubble;
