export class FilePreviewPolicy {
  static readonly TEXT_MAX_BYTES: number = 2 * 1024 * 1024;
  static readonly IMAGE_MAX_BYTES: number = 12 * 1024 * 1024;

  static textReadLimit(fileSize: number): number {
    return fileSize > 0 ? Math.min(fileSize, FilePreviewPolicy.TEXT_MAX_BYTES) :
      FilePreviewPolicy.TEXT_MAX_BYTES;
  }

  static canPreviewImage(fileSize: number): boolean {
    return fileSize <= FilePreviewPolicy.IMAGE_MAX_BYTES;
  }
}
