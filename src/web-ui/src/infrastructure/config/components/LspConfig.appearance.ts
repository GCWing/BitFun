import type { AppearanceSurfaceDescriptor } from '@/infrastructure/appearance';

export const lspConfigAppearanceDescriptor: AppearanceSurfaceDescriptor = {
  id: 'lsp-config',
  parts: [{ id: 'root' }, { id: 'message' }, { id: 'plugins' }],
};
