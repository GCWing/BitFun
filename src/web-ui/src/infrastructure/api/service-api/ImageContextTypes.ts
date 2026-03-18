// Shared image context payload shape passed from frontend to backend.
//
// Note: This is intentionally small and does not include the legacy image analysis APIs
// (`analyze_images`, `send_enhanced_message`). Image handling is done by the backend
// coordinator + tools (e.g. `view_image`).

export interface ImageContextData {
  id: string;
  image_path?: string;
  data_url?: string;
  mime_type: string;
  metadata?: Record<string, any>;
}

