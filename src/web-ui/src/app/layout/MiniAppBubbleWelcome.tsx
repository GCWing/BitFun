import React from 'react';
import { ArrowUpRight, FolderOpen } from 'lucide-react';
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
    <section className="bitfun-fmc__miniapp-welcome">
      <div
        className="bitfun-fmc__miniapp-welcome-icon"
        aria-hidden="true"
      >
        {renderMiniAppIcon(appIcon, 28)}
      </div>

      {title !== appName && (
        <div className="bitfun-fmc__miniapp-welcome-eyebrow">{appName}</div>
      )}
      <h2>{title}</h2>
      {description && <p className="bitfun-fmc__miniapp-welcome-description">{description}</p>}

      {workspaceLabel && workspacePath && (
        <div
          className="bitfun-fmc__miniapp-workspace"
          title={workspacePath}
          data-workspace-path={workspacePath}
        >
          <FolderOpen size={13} aria-hidden="true" />
          <span>{workspaceLabel}</span>
        </div>
      )}

      {suggestions.length > 0 && (
        <div className="bitfun-fmc__miniapp-suggestions">
          {welcome?.suggestionsLabel && (
            <div className="bitfun-fmc__miniapp-suggestions-label">
              {welcome.suggestionsLabel}
            </div>
          )}
          <div className="bitfun-fmc__miniapp-suggestions-list">
            {suggestions.map((suggestion, index) => (
              <button
                key={`${suggestion.label}:${index}`}
                type="button"
                className="bitfun-fmc__miniapp-suggestion"
                title={suggestion.prompt}
                onClick={() => onSuggestion(suggestion.prompt)}
              >
                <span>{suggestion.label}</span>
                <ArrowUpRight size={13} aria-hidden="true" />
              </button>
            ))}
          </div>
        </div>
      )}
    </section>
  );
};

export default MiniAppBubbleWelcome;
