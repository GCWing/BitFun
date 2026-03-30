import { PluginKey } from '@tiptap/pm/state';

export interface InlineAiPreviewLabels {
  title: string;
  streaming: string;
  ready: string;
  error: string;
  accept: string;
  reject: string;
  retry: string;
}

export interface InlineAiPreviewWidgetState {
  blockId: string;
  status: 'submitting' | 'streaming' | 'ready' | 'error';
  response: string;
  error: string | null;
  basePath?: string;
  canAccept: boolean;
  labels: InlineAiPreviewLabels;
  onAccept: () => void;
  onReject: () => void;
  onRetry: () => void;
}

export const inlineAiPreviewPluginKey = new PluginKey<InlineAiPreviewWidgetState | null>(
  'meditorInlineAiPreview'
);
