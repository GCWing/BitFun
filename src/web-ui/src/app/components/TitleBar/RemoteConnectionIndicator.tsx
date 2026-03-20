/**
 * Remote Connection Status Indicator
 * Shows when connected to a remote SSH workspace
 */

import React from 'react';
import { Tooltip } from '@/component-library';
import { useSSHRemoteContext } from '@/features/ssh-remote';

export const RemoteConnectionIndicator: React.FC = () => {
  const { remoteWorkspace, isConnected } = useSSHRemoteContext();

  if (!isConnected || !remoteWorkspace) {
    return null;
  }

  return (
    <Tooltip
      content={
        <div className="remote-connection-tooltip">
          <div className="remote-connection-tooltip-title">Remote SSH</div>
          <div className="remote-connection-tooltip-info">
            <span className="remote-connection-name">{remoteWorkspace.connectionName}</span>
            <span className="remote-connection-path">{remoteWorkspace.remotePath}</span>
          </div>
        </div>
      }
      placement="bottom"
    >
      <div className="remote-connection-indicator">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M9 3H5a2 2 0 0 0-2 2v4m6-6h10a2 2 0 0 1 2 2v4M9 3v18m0 0h10a2 2 0 0 0 2-2v-4M9 21H5a2 2 0 0 1-2-2v-4m0-6v6" />
        </svg>
        <span className="remote-connection-dot" />
      </div>
    </Tooltip>
  );
};

export default RemoteConnectionIndicator;
