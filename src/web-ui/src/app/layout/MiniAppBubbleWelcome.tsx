import React from 'react';
import { Icon, ScrollArea } from '@bitfun/ui';
import { FolderOpen } from 'lucide-react';
import type { MiniAppBubbleCustomization } from '@/app/scenes/miniapps/miniAppStore';
import { renderMiniAppIcon } from '@/app/scenes/miniapps/utils/miniAppIcons';

interface MiniAppBubbleWelcomeProps {
  appName: string;
  appDescription?: string;
  appIcon?: string;
  customization?: MiniAppBubbleCustomization;
  workspacePath?: string;
  onSuggestion: (prompt: string) => void;
}

/**
 * Host-rendered empty state for an Agentic MiniApp session. MiniApps provide a
 * bounded declarative model through app.chat.claimComposer; they never inject
 * markup into the shared conversation surface.
 */
export const MiniAppBubbleWelcome: React.FC<MiniAppBubbleWelcomeProps> = ({
  appName,
  appDescription,
  appIcon = 'Box',
  customization,
  workspacePath,
  onSuggestion,
}) => {
  const welcome = customization?.welcome;
  const title = welcome?.title || appName;
  const description = welcome?.description || appDescription;
  const workspaceLabel = welcome?.workspaceLabel || (workspacePath ? appName : '');
  const suggestions = welcome?.suggestions || [];

  return (
    <ScrollArea className="bitfun-fmc__miniapp-welcome" data-bf-component="miniapp-bubble-welcome" data-bf-part="root">
      <div
        className="bitfun-fmc__miniapp-welcome-icon"
        data-bf-component="miniapp-bubble-welcome"
        data-bf-part="icon"
        aria-hidden="true"
      >
        {renderMiniAppIcon(appIcon, 28)}
      </div>

      {title !== appName && (
        <div className="bitfun-fmc__miniapp-welcome-eyebrow" data-bf-component="miniapp-bubble-welcome" data-bf-part="eyebrow">{appName}</div>
      )}
      <h2 data-bf-component="miniapp-bubble-welcome" data-bf-part="title">{title}</h2>
      {description && <p className="bitfun-fmc__miniapp-welcome-description" data-bf-component="miniapp-bubble-welcome" data-bf-part="description">{description}</p>}

      {workspaceLabel && workspacePath && (
        <div
          className="bitfun-fmc__miniapp-workspace"
          data-bf-component="miniapp-bubble-welcome"
          data-bf-part="workspace"
          title={workspacePath}
          data-workspace-path={workspacePath}
        >
          <FolderOpen size={13} aria-hidden="true" />
          <span>{workspaceLabel}</span>
        </div>
      )}

      {suggestions.length > 0 && (
        <div className="bitfun-fmc__miniapp-suggestions" data-bf-component="miniapp-bubble-welcome" data-bf-part="suggestions">
          {welcome?.suggestionsLabel && (
            <div className="bitfun-fmc__miniapp-suggestions-label" data-bf-component="miniapp-bubble-welcome" data-bf-part="suggestionsLabel">
              {welcome.suggestionsLabel}
            </div>
          )}
          <div className="bitfun-fmc__miniapp-suggestions-list" data-bf-component="miniapp-bubble-welcome" data-bf-part="suggestionsList">
            {suggestions.map((suggestion, index) => (
              <button
                key={`${suggestion.label}:${index}`}
                type="button"
                className="bitfun-fmc__miniapp-suggestion"
                data-bf-component="miniapp-bubble-welcome"
                data-bf-part="suggestion"
                title={suggestion.prompt}
                onClick={() => onSuggestion(suggestion.prompt)}
              >
                <span>{suggestion.label}</span>
                <Icon name="arrow-up-right" size="xs" aria-hidden="true" />
              </button>
            ))}
          </div>
        </div>
      )}
    </ScrollArea>
  );
};

export default MiniAppBubbleWelcome;
