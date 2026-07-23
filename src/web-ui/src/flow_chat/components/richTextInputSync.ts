export type RichTextExternalSyncAction = 'noop' | 'clear' | 'replace';

export function getRichTextExternalSyncAction(
  value: string,
  currentContent: string
): RichTextExternalSyncAction {
  if (value === currentContent) {
    return 'noop';
  }

  if (!value) {
    // Always clear when value is empty so residual <br> nodes left by the browser
    // after deleting the last character are removed, allowing :empty::before to show.
    return 'clear';
  }

  return 'replace';
}
