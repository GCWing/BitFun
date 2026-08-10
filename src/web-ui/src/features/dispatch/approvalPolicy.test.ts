import { describe, expect, it } from 'vitest';
import {
  DISPATCH_PERMISSION_MODES,
  dispatchApprovalPolicyFromPermissionMode,
  dispatchApprovalPolicyFromSessionMode,
  permissionModeFromDispatchApprovalPolicy,
} from './approvalPolicy';

describe('dispatch approval policy', () => {
  it('starts a dispatch session on the stored local permission default', () => {
    expect(dispatchApprovalPolicyFromSessionMode('ask')).toBe('remote');
    expect(dispatchApprovalPolicyFromSessionMode('auto_approve')).toBe('auto');
    // Full access already resolves every ask, so nothing is worth forwarding.
    expect(dispatchApprovalPolicyFromSessionMode('full_access')).toBe('auto');
  });

  it('round-trips every mode the composer control offers', () => {
    for (const mode of DISPATCH_PERMISSION_MODES) {
      expect(
        permissionModeFromDispatchApprovalPolicy(
          dispatchApprovalPolicyFromPermissionMode(mode),
        ),
      ).toBe(mode);
    }
  });

  it('renders an unset policy as the mode that keeps the user in the loop', () => {
    // A projection restored from a record written before the policy existed
    // must not silently read as auto-approve.
    expect(permissionModeFromDispatchApprovalPolicy(undefined)).toBe('ask');
    expect(permissionModeFromDispatchApprovalPolicy('remote')).toBe('ask');
  });
});
