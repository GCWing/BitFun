import type { ImageContext } from '@/shared/types/context';
import type { ImageContextData as ImageInputContextData } from '@/infrastructure/api/service-api/ImageContextTypes';
import { api } from '@/infrastructure/api/service-api/ApiClient';

export interface ImageDisplayData {
  id: string;
  name: string;
  dataUrl?: string;
  imagePath?: string;
  mimeType?: string;
}

export interface ImagePayload {
  imageContexts: ImageInputContextData[];
  imageDisplayData: ImageDisplayData[];
}

export async function buildImagePayload(imageContexts: ImageContext[]): Promise<ImagePayload | undefined> {
  if (imageContexts.length === 0) {
    return undefined;
  }

  const clipboardImages = imageContexts.filter(ctx => !ctx.isLocal && ctx.dataUrl);
  const uploadedImagePaths = new Map<string, string>();

  if (clipboardImages.length > 0) {
    const uploadResults = await api.invoke<Array<{ id: string; image_path?: string | null }>>(
      'upload_image_contexts',
      {
        request: {
          images: clipboardImages.map(ctx => ({
            id: ctx.id,
            image_path: ctx.imagePath || null,
            data_url: ctx.dataUrl || null,
            mime_type: ctx.mimeType,
            image_name: ctx.imageName,
            file_size: ctx.fileSize,
            width: ctx.width || null,
            height: ctx.height || null,
            source: ctx.source,
          })),
        },
      }
    );

    for (const result of uploadResults) {
      if (result.image_path) {
        uploadedImagePaths.set(result.id, result.image_path);
      }
    }
  }

  return {
    imageContexts: imageContexts.map(ctx => ({
      id: ctx.id,
      image_path: ctx.isLocal ? ctx.imagePath : uploadedImagePaths.get(ctx.id),
      data_url: undefined,
      mime_type: ctx.mimeType,
      metadata: {
        name: ctx.imageName,
        width: ctx.width,
        height: ctx.height,
        file_size: ctx.fileSize,
        source: ctx.source,
      },
    })),
    imageDisplayData: imageContexts.map(ctx => ({
      id: ctx.id,
      name: ctx.imageName || 'Image',
      dataUrl: ctx.dataUrl,
      imagePath: ctx.isLocal ? ctx.imagePath : uploadedImagePaths.get(ctx.id),
      mimeType: ctx.mimeType,
    })),
  };
}
