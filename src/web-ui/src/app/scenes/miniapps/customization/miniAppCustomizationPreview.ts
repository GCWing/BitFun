export function getNextMiniAppPreviewOpenState(params: {
  hasPreview: boolean;
  isOpen: boolean;
}): boolean {
  if (!params.hasPreview) {
    return false;
  }

  return !params.isOpen;
}
