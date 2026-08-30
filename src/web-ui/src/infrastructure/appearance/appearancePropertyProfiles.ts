import type {
  AppearancePropertyProfile,
  AppearanceStyleProperty,
} from './types';

const PAINT_PROPERTIES = [
  'backgroundColor', 'backgroundImage', 'backgroundSize', 'backgroundPosition',
  'backgroundRepeat', 'backgroundImages', 'backgroundSizes', 'backgroundPositions',
  'backgroundRepeats', 'backgroundBlendModes', 'color', 'caretColor', 'accentColor',
  'textDecorationColor', 'borderColor', 'borderTopColor', 'borderRightColor',
  'borderBottomColor', 'borderLeftColor', 'borderWidth', 'borderTopWidth',
  'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth', 'borderStyle',
  'borderRadius', 'borderTopLeftRadius', 'borderTopRightRadius',
  'borderBottomRightRadius', 'borderBottomLeftRadius', 'outlineColor', 'outlineWidth',
  'outlineOffset', 'outlineStyle', 'boxShadow', 'opacity',
  'textAlign', 'verticalAlign', 'textIndent', 'textDecoration', 'textTransform',
  'textOverflow', 'whiteSpace', 'wordBreak', 'overflowWrap', 'cursor', 'transition',
  'transform', 'filter', 'backdropBlur', 'objectFit', 'objectPosition',
  'mixBlendMode', 'isolation',
] as const satisfies readonly AppearanceStyleProperty[];

const CONTROL_PROPERTIES = [
  ...PAINT_PROPERTIES,
  'display', 'flexDirection', 'flexWrap', 'alignItems', 'alignContent',
  'justifyContent',
  'boxSizing', 'width', 'minWidth', 'maxWidth', 'height', 'minHeight', 'maxHeight',
  'padding', 'paddingBlock', 'paddingInline', 'paddingTop', 'paddingRight',
  'paddingBottom', 'paddingLeft', 'gap', 'rowGap', 'columnGap', 'flexGrow',
  'flexShrink', 'flexBasis', 'alignSelf', 'aspectRatio',
] as const satisfies readonly AppearanceStyleProperty[];

const CONTAINER_PROPERTIES = [
  ...CONTROL_PROPERTIES,
  'justifySelf', 'justifyItems', 'placeItems', 'placeContent',
  'gridTemplateColumns', 'gridTemplateRows', 'gridAutoFlow', 'gridColumnSpan',
  'gridRowSpan', 'margin', 'marginBlock', 'marginInline', 'marginTop', 'marginRight',
  'marginBottom', 'marginLeft', 'overflow', 'overflowX', 'overflowY',
] as const satisfies readonly AppearanceStyleProperty[];

const LAYOUT_PROPERTIES = [
  ...CONTAINER_PROPERTIES,
  'order', 'position', 'insetBlock', 'insetInline', 'top', 'right', 'bottom',
  'left', 'zIndex',
] as const satisfies readonly AppearanceStyleProperty[];

const PROFILE_PROPERTIES: Record<AppearancePropertyProfile, readonly AppearanceStyleProperty[]> = {
  paint: PAINT_PROPERTIES,
  control: CONTROL_PROPERTIES,
  container: CONTAINER_PROPERTIES,
  layout: LAYOUT_PROPERTIES,
  overlay: LAYOUT_PROPERTIES,
};

export const DEFAULT_APPEARANCE_PROPERTY_PROFILE: AppearancePropertyProfile = 'container';

export function getAppearanceProfileProperties(
  profile: AppearancePropertyProfile = DEFAULT_APPEARANCE_PROPERTY_PROFILE,
): readonly AppearanceStyleProperty[] {
  return PROFILE_PROPERTIES[profile];
}

export function getDefaultForceableAppearanceProperties(): readonly AppearanceStyleProperty[] {
  return PAINT_PROPERTIES;
}
