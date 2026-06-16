/**
 * TerminalScene — renders a ConnectedTerminal for the session selected
 * via terminalSceneStore (set from the Shell navigation).
 *
 * When no session is active, shows a minimal empty state prompting the
 * user to open a terminal from the navigation panel.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { SquareTerminal } from 'lucide-react';
import { useTerminalSceneStore } from '../../stores/terminalSceneStore';
import ConnectedTerminal from '../../../tools/terminal/components/ConnectedTerminal';
import './TerminalScene.scss';

interface TerminalSceneProps {
  isActive?: boolean;
}

const TerminalScene: React.FC<TerminalSceneProps> = ({ isActive = true }) => {
  const { activeSessionId, setActiveSession } = useTerminalSceneStore();
  const { t } = useTranslation('panels/terminal');

  const handleExit = useCallback(() => {
    setActiveSession(null);
  }, [setActiveSession]);

  const handleClose = useCallback(() => {
    setActiveSession(null);
  }, [setActiveSession]);

  // Keep the ConnectedTerminal mounted when the scene is inactive. Unmounting
  // would dispose xterm and force replay on return, which can lose scrollback
  // and cursor state after resize-sensitive shell output.
  return (
    <div className="bitfun-terminal-scene" aria-hidden={!isActive}>
      {activeSessionId ? (
        <ConnectedTerminal
          key={activeSessionId}
          sessionId={activeSessionId}
          autoFocus={isActive}
          showToolbar
          showStatusBar
          onExit={handleExit}
          onClose={handleClose}
        />
      ) : (
        <div className="bitfun-terminal-scene__empty">
          <SquareTerminal size={32} className="bitfun-terminal-scene__empty-icon" />
          <p className="bitfun-terminal-scene__empty-hint">{t('emptyState')}</p>
        </div>
      )}
    </div>
  );
};

export default TerminalScene;
