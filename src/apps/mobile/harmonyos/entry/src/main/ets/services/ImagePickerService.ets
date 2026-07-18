import fs from '@ohos.file.fs';
import image from '@ohos.multimedia.image';
import photoAccessHelper from '@ohos.file.photoAccessHelper';
import util from '@ohos.util';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { RemoteImageContext, SelectedImageAttachment } from '../model/RemoteModels';

interface ImageMetadata {
  name: string;
  size: number;
  source_uri: string;
}

interface EncodedImage {
  bytes: Uint8Array;
  mimeType: string;
  compressed: boolean;
}

interface ImageSize {
  width: number;
  height: number;
}

export class ImagePickerService {
  private readonly maxImageBytes: number = 4 * 1024 * 1024;
  private readonly maxImageDimension: number = 1600;
  private readonly compressionQualities: number[] = [82, 72, 62];

  async pickImages(maxCount: number, existingCount: number): Promise<SelectedImageAttachment[]> {
    const remaining = Math.max(0, maxCount - existingCount);
    if (remaining === 0) {
      return [];
    }

    const options = new photoAccessHelper.PhotoSelectOptions();
    options.MIMEType = photoAccessHelper.PhotoViewMIMETypes.IMAGE_TYPE;
    options.maxSelectNumber = remaining;
    options.isOriginalSupported = false;

    const picker = new photoAccessHelper.PhotoViewPicker();
    const result = await picker.select(options);
    const uris = result.photoUris || [];
    const selected: SelectedImageAttachment[] = [];
    for (let index = 0; index < uris.length; index += 1) {
      selected.push(await this.readImage(uris[index], index));
    }
    return selected;
  }

  toRemoteContexts(images: SelectedImageAttachment[]): RemoteImageContext[] {
    return images.map((attachment: SelectedImageAttachment) => {
      const metadata: ImageMetadata = {
        name: attachment.name,
        size: attachment.size,
        source_uri: attachment.uri
      };
      const context: RemoteImageContext = {
        id: attachment.id,
        data_url: attachment.data_url,
        mime_type: attachment.mime_type,
        metadata
      };
      return context;
    });
  }

  private async readImage(uri: string, index: number): Promise<SelectedImageAttachment> {
    const stat = fs.statSync(uri);
    const encoded = await this.encodeImage(uri, stat.size);
    if (encoded.bytes.length > this.maxImageBytes) {
      throw new Error(RemoteI18n.f('errors.imageTooLargeAfterCompression', `${Math.round(this.maxImageBytes / 1024 / 1024)}`));
    }

    const base64 = new util.Base64Helper().encodeToStringSync(encoded.bytes);
    const name = this.imageName(uri, index);
    return {
      id: `harmony-image-${Date.now()}-${index}`,
      uri,
      name,
      data_url: `data:${encoded.mimeType};base64,${base64}`,
      mime_type: encoded.mimeType,
      size: encoded.bytes.length,
      original_size: stat.size,
      compressed: encoded.compressed
    };
  }

  private async encodeImage(uri: string, originalSize: number): Promise<EncodedImage> {
    if (originalSize <= this.maxImageBytes) {
      return this.readOriginalImage(uri, originalSize);
    }

    try {
      return await this.compressImage(uri, originalSize);
    } catch (_err) {
      if (originalSize <= this.maxImageBytes) {
        return this.readOriginalImage(uri, originalSize);
      }
      throw new Error(RemoteI18n.t('errors.imageCompressionFailed'));
    }
  }

  private readOriginalImage(uri: string, originalSize: number): EncodedImage {
    const file = fs.openSync(uri);
    try {
      const buffer = new ArrayBuffer(originalSize);
      const readSize = fs.readSync(file.fd, buffer, { length: originalSize });
      const bytes = new Uint8Array(buffer);
      const payload = readSize === bytes.length ? bytes : bytes.slice(0, readSize);
      return {
        bytes: payload,
        mimeType: this.inferMimeType(uri),
        compressed: false
      };
    } finally {
      fs.closeSync(file);
    }
  }

  private async compressImage(uri: string, originalSize: number): Promise<EncodedImage> {
    const file = fs.openSync(uri);
    let source: image.ImageSource | undefined = undefined;
    let pixelMap: image.PixelMap | undefined = undefined;
    let packer: image.ImagePacker | undefined = undefined;

    try {
      source = image.createImageSource(file.fd);
      const info = await source.getImageInfo();
      const targetSize = this.targetSize(info.size.width, info.size.height);
      pixelMap = await source.createPixelMap({ desiredSize: targetSize });

      packer = image.createImagePacker();
      let bestBytes: Uint8Array | undefined = undefined;
      for (let index = 0; index < this.compressionQualities.length; index += 1) {
        const packed = await packer.packToData(pixelMap, {
          format: 'image/jpeg',
          quality: this.compressionQualities[index],
          bufferSize: this.maxImageBytes + 1024 * 1024
        });
        const bytes = new Uint8Array(packed);
        bestBytes = bytes;
        if (bytes.length <= this.maxImageBytes) {
          return {
            bytes,
            mimeType: 'image/jpeg',
            compressed: true
          };
        }
      }

      return {
        bytes: bestBytes || new Uint8Array(0),
        mimeType: 'image/jpeg',
        compressed: true
      };
    } finally {
      await this.releasePacker(packer);
      await this.releasePixelMap(pixelMap);
      await this.releaseSource(source);
      fs.closeSync(file);
    }
  }

  private targetSize(width: number, height: number): ImageSize {
    const longest = Math.max(width, height);
    if (longest <= this.maxImageDimension || longest <= 0) {
      return { width, height };
    }

    const ratio = this.maxImageDimension / longest;
    return {
      width: Math.max(1, Math.round(width * ratio)),
      height: Math.max(1, Math.round(height * ratio))
    };
  }

  private async releaseSource(source: image.ImageSource | undefined): Promise<void> {
    if (!source) {
      return;
    }
    try {
      await source.release();
    } catch (_err) {
    }
  }

  private async releasePixelMap(pixelMap: image.PixelMap | undefined): Promise<void> {
    if (!pixelMap) {
      return;
    }
    try {
      await pixelMap.release();
    } catch (_err) {
    }
  }

  private async releasePacker(packer: image.ImagePacker | undefined): Promise<void> {
    if (!packer) {
      return;
    }
    try {
      await packer.release();
    } catch (_err) {
    }
  }

  private inferMimeType(uri: string): string {
    const lower = uri.toLowerCase();
    if (lower.indexOf('.png') >= 0) {
      return 'image/png';
    }
    if (lower.indexOf('.webp') >= 0) {
      return 'image/webp';
    }
    if (lower.indexOf('.gif') >= 0) {
      return 'image/gif';
    }
    return 'image/jpeg';
  }

  private imageName(uri: string, index: number): string {
    const cleanUri = uri.split('?')[0];
    const parts = cleanUri.replace(/\\/g, '/').split('/');
    const raw = parts[parts.length - 1] || `image-${index + 1}`;
    try {
      return decodeURIComponent(raw);
    } catch (_err) {
      return raw;
    }
  }
}
