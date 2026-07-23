export const REMOTE_CONNECT_DISCLAIMER_KEY = 'bitfun:remote-connect:disclaimer-agreed:v1';

export const getRemoteConnectDisclaimerAgreed = (): boolean => {
  try {
    return localStorage.getItem(REMOTE_CONNECT_DISCLAIMER_KEY) === 'true';
  } catch {
    return false;
  }
};

export const setRemoteConnectDisclaimerAgreed = (): void => {
  try {
    localStorage.setItem(REMOTE_CONNECT_DISCLAIMER_KEY, 'true');
  } catch {
    // Ignore storage failures and fall back to in-memory state.
  }
};
