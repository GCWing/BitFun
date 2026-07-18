/**
 * Host for the imperative peer directory picker store.
 */

import React from 'react';
import { PeerDirectoryBrowser } from './PeerDirectoryBrowser';
import { usePeerDirectoryPickerStore } from './peerDirectoryPickerStore';

export const PeerDirectoryPickerHost: React.FC = () => {
  const isOpen = usePeerDirectoryPickerStore((s) => s.isOpen);
  const title = usePeerDirectoryPickerStore((s) => s.title);
  const defaultPath = usePeerDirectoryPickerStore((s) => s.defaultPath);
  const select = usePeerDirectoryPickerStore((s) => s.select);
  const cancel = usePeerDirectoryPickerStore((s) => s.cancel);

  if (!isOpen) {
    return null;
  }

  return (
    <PeerDirectoryBrowser
      title={title}
      initialPath={defaultPath}
      onSelect={select}
      onCancel={cancel}
    />
  );
};

export default PeerDirectoryPickerHost;
