/**
 * Component library type definitions
 */
export interface ComponentPreview {
  id: string;
  name: string;
  description: string;
  category: string;
  component: React.ComponentType<any>;
  props?: Record<string, any>;
}

export type LayoutType = 
  | 'full-page'    // Full-page preview (design system)
  | 'large-card'   // Single-column large card (content components)
  | 'demo'         // Live demo (feedback components)
  | 'column'       // Columns with quick navigation
  | 'grid-2'       // Two-column grid (form components)
  | 'grid-3'       // Three-column grid (standard showcase)
  | 'grid-4'       // Four-column grid (small components)
  | 'default';     // Default layout

export interface ComponentCategory {
  id: string;
  name: string;
  description: string;
  components: ComponentPreview[];
  layoutType?: LayoutType;
}