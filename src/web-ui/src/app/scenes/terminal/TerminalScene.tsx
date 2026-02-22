/**
 * TerminalScene — renders a ConnectedTerminal for the session selected
 * via terminalSceneStore (set from NavPanel Shell list or Shell Hub).
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

const TerminalScene: React.FC = () => {
  const { activeSessionId, setActiveSession } = useTerminalSceneStore();
  const { t } = useTranslation('panels/terminal');

  const handleExit = useCallback(() => {
    setActiveSession(null);
  }, [setActiveSession]);

  const handleClose = useCallback(() => {
    setActiveSession(null);
  }, [setActiveSession]);

  return (
    <div className="bitfun-terminal-scene">
      {activeSessionId ? (
        <ConnectedTerminal
          key={activeSessionId}
          sessionId={activeSessionId}
          autoFocus
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
