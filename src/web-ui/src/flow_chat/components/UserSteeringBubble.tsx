/**
 * Renders a `user-steering` flow item as a normal user message in the
 * conversation flow. The backend confirmation still updates this item by
 * `steeringId`, but the user-facing surface is intentionally identical to a
 * message sent from the composer — attachments included.
 *
 * The item is appended to the *current* model round's items, so it visually
 * sits after whatever thinking / text / tool-call content has already
 * streamed. When the backend finishes the current atomic step and starts a
 * new model round, that next round renders below it — matching the user's
 * mental model of "the agent reads my steering and responds in a new turn".
 */

import { UserMessage } from './UserMessage';
import type { FlowUserSteeringItem, SteeringImage } from '../types/flow-chat';
import './UserSteeringBubble.scss';

interface UserSteeringBubbleProps {
  item: FlowUserSteeringItem;
}

function imageSource(image: SteeringImage): string | undefined {
  if (image.dataUrl) return image.dataUrl;
  if (image.imagePath) {
    return `https://asset.localhost/${encodeURIComponent(image.imagePath)}`;
  }
  return undefined;
}

export function UserSteeringBubble({ item }: UserSteeringBubbleProps): JSX.Element {
  const images = item.images ?? [];
  return (
    <>
      <UserMessage
        message={item.content}
        timestamp={item.timestamp}
      />
      {images.length > 0 && (
        <div
          data-bf-component="user-steering-bubble"
          data-bf-part="images"
          className="bitfun-user-steering-bubble__images"
        >
          {images.map(image => {
            const src = imageSource(image);
            return src ? (
              <div
                data-bf-component="user-steering-bubble"
                data-bf-part="image"
                key={image.id}
                className="bitfun-user-steering-bubble__image"
              >
                <img src={src} alt={image.name ?? image.id} />
              </div>
            ) : null;
          })}
        </div>
      )}
    </>
  );
}

export default UserSteeringBubble;
