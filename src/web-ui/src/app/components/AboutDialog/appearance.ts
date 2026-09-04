import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const aboutDialogAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'about-dialog',
  parts: [
    // Modal owns the dialog shell; this root is content inside that shell.
    { id: 'root', visualRole: 'content', continuityGroup: 'about-dialog' },
    { id: 'hero', visualRole: 'content', continuityGroup: 'about-dialog' },
    { id: 'title', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'content', visualRole: 'content', continuityGroup: 'about-dialog' },
    { id: 'channelBadge', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'updateCard', visualRole: 'content' },
    { id: 'updateFeedback', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'updateActions', visualRole: 'toolbar' },
    { id: 'progress', propertyProfile: 'control', visualRole: 'control' },
    { id: 'progressFill', propertyProfile: 'paint', visualRole: 'decoration' },
    { id: 'infoRow', visualRole: 'content' },
    { id: 'infoLabel', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'infoValue', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'copyButton', propertyProfile: 'control', visualRole: 'control' },
    { id: 'starCallout', visualRole: 'toolbar', continuityGroup: 'about-dialog' },
    { id: 'footer', visualRole: 'toolbar', continuityGroup: 'about-dialog' },
    { id: 'license', propertyProfile: 'paint', visualRole: 'content' },
    { id: 'copyright', propertyProfile: 'paint', visualRole: 'content' },
  ],
  states: [
    { id: 'checking', selector: { kind: 'ancestorPart', part: 'updateCard', suffix: '[data-openbitfun-state~="checking"]' } },
    { id: 'latest', selector: { kind: 'ancestorPart', part: 'updateCard', suffix: '[data-openbitfun-state~="latest"]' } },
    { id: 'downloading', selector: { kind: 'ancestorPart', part: 'updateCard', suffix: '[data-openbitfun-state~="downloading"]' } },
    { id: 'installed', selector: { kind: 'ancestorPart', part: 'updateCard', suffix: '[data-openbitfun-state~="installed"]' } },
    { id: 'error', selector: { kind: 'ancestorPart', part: 'updateCard', suffix: '[data-openbitfun-state~="error"]' } },
  ],
};
