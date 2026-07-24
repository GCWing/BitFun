/**
 * Lets nested typewriter consumers report whether they are still revealing,
 * so parents (e.g. ModelRoundItem footer) can wait for visual completion.
 */

import React, {
  type ReactNode,
} from 'react';
import {
  TypewriterRevealGateContext,
  useCreateTypewriterRevealGate,
  type TypewriterRevealGateValue,
} from './typewriterRevealGateContext';

export const TypewriterRevealGateProvider: React.FC<{
  value?: TypewriterRevealGateValue;
  children: ReactNode;
}> = ({ value, children }) => {
  const localValue = useCreateTypewriterRevealGate();
  return (
    <TypewriterRevealGateContext.Provider value={value ?? localValue}>
      {children}
    </TypewriterRevealGateContext.Provider>
  );
};
