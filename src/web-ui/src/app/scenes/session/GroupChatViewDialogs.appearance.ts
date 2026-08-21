import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const groupMemberPickerDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-member-picker-dialog',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'dialog' },
  ],
};

export const groupForkDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-fork-dialog',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'dialog' },
  ],
};

export const groupMemberListDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'group-member-list-dialog',
  parts: [
    { id: 'root', propertyProfile: 'layout', visualRole: 'dialog' },
    { id: 'memberRow', visualRole: 'content' },
  ],
};
