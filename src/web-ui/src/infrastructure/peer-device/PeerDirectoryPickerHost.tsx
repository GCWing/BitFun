/**
 * Host for the imperative peer directory picker store.
 */

import React, { useRef } from 'react';
import { PresenceBoundary } from '@/component-library';
import { PeerDirectoryBrowser } from './PeerDirectoryBrowser';
import { usePeerDirectoryPickerStore } from './peerDirectoryPickerStore';

export const PeerDirectoryPickerHost: React.FC = () => {
  const isOpen = usePeerDirectoryPickerStore((s) => s.isOpen);
  const title = usePeerDirectoryPickerStore((s) => s.title);
  const defaultPath = usePeerDirectoryPickerStore((s) => s.defaultPath);
  const requestResolver = usePeerDirectoryPickerStore((s) => s.resolve);
  const select = usePeerDirectoryPickerStore((s) => s.select);
  const cancel = usePeerDirectoryPickerStore((s) => s.cancel);
  const retainedPickerRef = useRef({ title, defaultPath });
  const requestIdentityRef = useRef<{
    resolver: typeof requestResolver;
    key: number;
  }>({ resolver: null, key: 0 });

  if (isOpen) {
    retainedPickerRef.current = { title, defaultPath };
    if (requestIdentityRef.current.resolver !== requestResolver) {
      requestIdentityRef.current = {
        resolver: requestResolver,
        key: requestIdentityRef.current.key + 1,
      };
    }
  }

  return (
    <PresenceBoundary active={isOpen}>
      <PeerDirectoryBrowser
        key={requestIdentityRef.current.key}
        visible={isOpen}
        title={retainedPickerRef.current.title}
        initialPath={retainedPickerRef.current.defaultPath}
        onSelect={select}
        onCancel={cancel}
      />
    </PresenceBoundary>
  );
};

export default PeerDirectoryPickerHost;
