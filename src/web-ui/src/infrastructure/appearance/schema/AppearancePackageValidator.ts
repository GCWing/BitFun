import type { AppearanceRegistry } from '../registry/AppearanceRegistry';
import {
  APPEARANCE_SCHEMA,
  APPEARANCE_SCHEMA_VERSION,
  type AppearancePackage,
  type AppearanceStyle,
  type AppearanceStyleProperty,
  type AppearanceSurfaceDefinition,
  type AppearanceSurfaceDescriptor,
  type AppearanceValidationIssue,
  type AppearanceValidationResult,
} from '../types';
import { getAppearanceProfileProperties } from '../appearancePropertyProfiles';
import { AppearancePackageValidationError } from './AppearancePackageValidationError';

type ValidationErrorReporter = (
  path: string,
  code: string,
  message: string,
  context?: AppearanceValidationIssue['context'],
) => void;

const ID_PATTERN = /^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)*$/;
const VERSION_PATTERN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const REFERENCE_PATTERN = /^globals\.(colors|lengths|numbers|durations|easings|shadows)\.[a-z][a-zA-Z0-9.-]*$/;
type AppearanceTokenGroup = 'colors' | 'lengths' | 'numbers' | 'durations' | 'easings' | 'shadows';
const FORBIDDEN_TEXT_PATTERN = /(?:https?:\/\/|javascript:|data:|url\s*\(|<\/?[a-z]|[{};])/i;
const VISUAL_ROLES = new Set([
  'workspace', 'continuous-surface', 'panel', 'toolbar', 'card', 'control',
  'popup', 'dialog', 'divider', 'content', 'decoration',
]);

const STYLE_PROPERTIES = new Set<AppearanceStyleProperty>([
  'backgroundColor', 'backgroundImage', 'color', 'caretColor', 'accentColor',
  'textDecorationColor', 'borderColor', 'borderTopColor',
  'backgroundSize', 'backgroundPosition', 'backgroundRepeat',
  'backgroundImages', 'backgroundSizes', 'backgroundPositions',
  'backgroundRepeats', 'backgroundBlendModes',
  'borderRightColor', 'borderBottomColor', 'borderLeftColor', 'borderWidth',
  'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth',
  'borderStyle', 'borderRadius', 'borderTopLeftRadius', 'borderTopRightRadius',
  'borderBottomRightRadius', 'borderBottomLeftRadius', 'boxSizing',
  'outlineColor', 'outlineWidth', 'outlineOffset', 'outlineStyle',
  'boxShadow', 'opacity', 'textAlign', 'verticalAlign',
  'textIndent', 'textDecoration',
  'textTransform', 'textOverflow', 'whiteSpace', 'wordBreak', 'overflowWrap', 'display',
  'flexDirection', 'flexWrap', 'flexGrow', 'flexShrink', 'flexBasis', 'alignItems',
  'alignContent', 'alignSelf', 'justifyContent', 'justifySelf', 'justifyItems',
  'placeItems', 'placeContent', 'order', 'gridTemplateColumns',
  'gridTemplateRows', 'gridAutoFlow', 'gridColumnSpan', 'gridRowSpan',
  'gap', 'rowGap', 'columnGap',
  'width', 'minWidth', 'maxWidth', 'height', 'minHeight', 'maxHeight', 'padding',
  'paddingBlock', 'paddingInline', 'paddingTop', 'paddingRight', 'paddingBottom',
  'paddingLeft', 'margin', 'marginBlock', 'marginInline', 'marginTop', 'marginRight',
  'marginBottom', 'marginLeft', 'position', 'insetBlock', 'insetInline', 'top',
  'right', 'bottom', 'left', 'zIndex', 'aspectRatio',
  'overflow', 'overflowX', 'overflowY', 'cursor', 'transition', 'transform',
  'filter', 'backdropBlur', 'objectFit', 'objectPosition', 'mixBlendMode', 'isolation',
]);

const COLOR_PROPERTIES = new Set<AppearanceStyleProperty>([
  'backgroundColor', 'color', 'caretColor', 'accentColor', 'textDecorationColor',
  'borderColor', 'borderTopColor', 'borderRightColor', 'borderBottomColor',
  'borderLeftColor', 'outlineColor',
]);
const LENGTH_PROPERTIES = new Set<AppearanceStyleProperty>([
  'borderWidth', 'borderTopWidth', 'borderRightWidth', 'borderBottomWidth',
  'borderLeftWidth', 'borderRadius', 'outlineWidth', 'outlineOffset',
  'borderTopLeftRadius', 'borderTopRightRadius', 'borderBottomRightRadius',
  'borderBottomLeftRadius',
  'textIndent', 'gap', 'rowGap', 'columnGap', 'width', 'minWidth', 'maxWidth',
  'height', 'minHeight', 'maxHeight', 'padding', 'paddingBlock', 'paddingInline',
  'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft', 'margin',
  'marginBlock', 'marginInline', 'marginTop', 'marginRight', 'marginBottom',
  'marginLeft', 'flexBasis', 'insetBlock', 'insetInline', 'top', 'right', 'bottom',
  'left', 'backdropBlur',
]);
const NUMBER_PROPERTIES = new Set<AppearanceStyleProperty>([
  'opacity', 'flexGrow', 'flexShrink', 'gridColumnSpan',
  'gridRowSpan', 'zIndex', 'order',
]);

const ENUM_VALUES: Partial<Record<AppearanceStyleProperty, ReadonlySet<string>>> = {
  borderStyle: new Set(['none', 'solid', 'dashed', 'dotted']),
  outlineStyle: new Set(['none', 'solid', 'dashed', 'dotted']),
  backgroundSize: new Set(['auto', 'cover', 'contain']),
  backgroundPosition: new Set(['center', 'top', 'right', 'bottom', 'left']),
  backgroundRepeat: new Set(['repeat', 'repeat-x', 'repeat-y', 'no-repeat']),
  boxSizing: new Set(['content-box', 'border-box']),
  textAlign: new Set(['left', 'center', 'right', 'start', 'end']),
  verticalAlign: new Set(['baseline', 'sub', 'super', 'text-top', 'text-bottom', 'middle', 'top', 'bottom']),
  textDecoration: new Set(['none', 'underline', 'line-through']),
  textTransform: new Set(['none', 'uppercase', 'lowercase', 'capitalize']),
  textOverflow: new Set(['clip', 'ellipsis']),
  whiteSpace: new Set(['normal', 'nowrap', 'pre', 'pre-wrap', 'pre-line']),
  wordBreak: new Set(['normal', 'break-all', 'keep-all', 'break-word']),
  overflowWrap: new Set(['normal', 'break-word', 'anywhere']),
  display: new Set(['block', 'inline', 'inline-block', 'flex', 'inline-flex', 'grid']),
  flexDirection: new Set(['row', 'row-reverse', 'column', 'column-reverse']),
  flexWrap: new Set(['nowrap', 'wrap', 'wrap-reverse']),
  alignItems: new Set(['stretch', 'flex-start', 'center', 'flex-end', 'baseline']),
  alignContent: new Set(['stretch', 'flex-start', 'center', 'flex-end', 'space-between', 'space-around', 'space-evenly']),
  alignSelf: new Set(['auto', 'stretch', 'flex-start', 'center', 'flex-end', 'baseline']),
  justifyContent: new Set(['flex-start', 'center', 'flex-end', 'space-between', 'space-around', 'space-evenly']),
  justifySelf: new Set(['auto', 'stretch', 'start', 'center', 'end']),
  justifyItems: new Set(['auto', 'normal', 'stretch', 'start', 'center', 'end']),
  placeItems: new Set(['normal', 'stretch', 'start', 'center', 'end']),
  placeContent: new Set(['normal', 'stretch', 'start', 'center', 'end', 'space-between', 'space-around', 'space-evenly']),
  gridAutoFlow: new Set(['row', 'column', 'dense', 'row dense', 'column dense']),
  position: new Set(['static', 'relative', 'absolute', 'fixed', 'sticky']),
  overflow: new Set(['visible', 'hidden', 'clip', 'auto', 'scroll']),
  overflowX: new Set(['visible', 'hidden', 'clip', 'auto', 'scroll']),
  overflowY: new Set(['visible', 'hidden', 'clip', 'auto', 'scroll']),
  cursor: new Set(['default', 'pointer', 'text', 'move', 'grab', 'grabbing', 'wait', 'not-allowed']),
  objectFit: new Set(['fill', 'contain', 'cover', 'none', 'scale-down']),
  objectPosition: new Set(['center', 'top', 'right', 'bottom', 'left']),
  mixBlendMode: new Set(['normal', 'multiply', 'screen', 'overlay', 'darken', 'lighten', 'soft-light', 'hard-light']),
  isolation: new Set(['auto', 'isolate']),
};

const BACKGROUND_SIZE_VALUES = new Set(['auto', 'cover', 'contain']);
const BACKGROUND_POSITION_VALUES = new Set(['center', 'top', 'right', 'bottom', 'left']);
const BACKGROUND_REPEAT_VALUES = new Set(['repeat', 'repeat-x', 'repeat-y', 'no-repeat']);
const BACKGROUND_BLEND_MODE_VALUES = new Set([
  'normal', 'multiply', 'screen', 'overlay', 'darken', 'lighten', 'soft-light', 'hard-light',
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function finiteNumber(value: unknown, min: number, max: number): boolean {
  return typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
}

export class AppearancePackageValidator {
  validate(input: unknown, registry: AppearanceRegistry): AppearanceValidationResult {
    const errors: AppearanceValidationIssue[] = [];
    const warnings: AppearanceValidationIssue[] = [];
    const error: ValidationErrorReporter = (path, code, message, context) => errors.push({
      path,
      code,
      message,
      ...(context ? { context } : {}),
    });
    const warning = (path: string, code: string, message: string) => warnings.push({ path, code, message });

    if (!isRecord(input)) {
      error('$', 'INVALID_PACKAGE', 'Appearance package must be an object');
      return { valid: false, errors, warnings };
    }
    this.validateKnownKeys(input, [
      'schema', 'schemaVersion', 'id', 'name', 'author', 'description', 'version', 'mode',
      'preview', 'backgroundMedia', 'requiredCapabilities', 'globals', 'materials', 'components', 'scenes', 'renderers',
      'assets', 'integrity',
    ], '$', error);

    if (input.schema !== APPEARANCE_SCHEMA) {
      error('schema', 'INVALID_SCHEMA', `Schema must be ${APPEARANCE_SCHEMA}`);
    }
    if (input.schemaVersion !== APPEARANCE_SCHEMA_VERSION) {
      error('schemaVersion', 'UNSUPPORTED_SCHEMA_VERSION', `Schema version must be ${APPEARANCE_SCHEMA_VERSION}`);
    }
    this.validateId(input.id, 'id', error);
    if (typeof input.name !== 'string' || input.name.trim().length === 0 || input.name.length > 100) {
      error('name', 'INVALID_NAME', 'Name must be between 1 and 100 characters');
    }
    for (const field of ['author', 'description'] as const) {
      const value = input[field];
      if (value !== undefined && (typeof value !== 'string' || value.length > (field === 'author' ? 100 : 500))) {
        error(field, `INVALID_${field.toUpperCase()}`, `Invalid ${field}`);
      }
    }
    if (typeof input.version !== 'string' || !VERSION_PATTERN.test(input.version)) {
      error('version', 'INVALID_VERSION', 'Version must use semantic version syntax');
    }
    if (input.mode !== 'light' && input.mode !== 'dark') {
      error('mode', 'INVALID_MODE', 'Mode must be light or dark');
    }

    if (input.requiredCapabilities !== undefined) {
      if (!Array.isArray(input.requiredCapabilities)) {
        error('requiredCapabilities', 'INVALID_CAPABILITIES', 'Required capabilities must be an array');
      } else {
        const supported = new Set(['components.v1', 'scenes.v1', 'renderers.v1', 'assets.v1', 'background-media.v1']);
        input.requiredCapabilities.forEach((capability, index) => {
          if (typeof capability !== 'string' || !supported.has(capability)) {
            error(`requiredCapabilities.${index}`, 'UNSUPPORTED_CAPABILITY', `Unsupported capability: ${String(capability)}`);
          }
        });
      }
    }

    this.validateGlobals(input.globals, 'globals', error);
    this.validateMaterials(input.materials, 'materials', error, warning);
    this.validateSurfaces(input.components, 'components', id => registry.getComponent(id), input.materials, error, warning);
    this.validateSurfaces(input.scenes, 'scenes', id => registry.getScene(id), input.materials, error, warning);
    this.validateRenderers(input.renderers, registry, error);
    this.validateAssets(input.assets, error);
    this.validatePreview(input.preview, input.assets, error);
    this.validateBackgroundMedia(input.backgroundMedia, input.assets, error);
    this.validateVideoAssetUsage(input.backgroundMedia, input.assets, input.requiredCapabilities, error);
    this.validateAssetReferences(input.materials, input.components, input.scenes, input.assets, error);
    this.validateIntegrity(input.integrity, input.assets, error);
    this.auditCascadeUsage(input.components, input.scenes, warning);
    this.validateReferenceGraph(input, error);

    return {
      valid: errors.length === 0,
      errors,
      warnings,
      value: errors.length === 0 ? input as unknown as AppearancePackage : undefined,
    };
  }

  private validatePreview(
    value: unknown,
    assets: unknown,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value) || value.kind !== 'asset' || typeof value.assetId !== 'string') {
      error('preview', 'INVALID_PREVIEW', 'Preview must reference a declared image asset');
      return;
    }
    if (!isRecord(assets) || !(value.assetId in assets)) {
      error('preview.assetId', 'UNKNOWN_ASSET_REFERENCE', `Unknown appearance asset ${value.assetId}`);
      return;
    }
    const asset = assets[value.assetId];
    if (!isRecord(asset) || asset.kind !== 'image') {
      error('preview.assetId', 'PREVIEW_ASSET_NOT_IMAGE', 'Preview must reference an image asset');
    }
  }

  private validateBackgroundMedia(
    value: unknown,
    assets: unknown,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value) || value.kind !== 'video') {
      error('backgroundMedia', 'INVALID_BACKGROUND_MEDIA', 'Background media must be a video declaration');
      return;
    }
    this.validateKnownKeys(value, ['kind', 'assetId', 'posterAssetId', 'fit', 'position'], 'backgroundMedia', error);
    const validateAssetKind = (field: 'assetId' | 'posterAssetId', expectedKind: 'video' | 'image'): void => {
      const assetId = value[field];
      if (typeof assetId !== 'string') {
        error(`backgroundMedia.${field}`, 'INVALID_BACKGROUND_MEDIA_ASSET', `${field} must reference a declared ${expectedKind} asset`);
        return;
      }
      if (!isRecord(assets) || !(assetId in assets)) {
        error(`backgroundMedia.${field}`, 'UNKNOWN_ASSET_REFERENCE', `Unknown appearance asset ${assetId}`);
        return;
      }
      const asset = assets[assetId];
      if (!isRecord(asset) || asset.kind !== expectedKind) {
        error(`backgroundMedia.${field}`, 'BACKGROUND_MEDIA_ASSET_KIND_MISMATCH', `${field} must reference a ${expectedKind} asset`);
      }
    };
    validateAssetKind('assetId', 'video');
    validateAssetKind('posterAssetId', 'image');
    if (value.fit !== undefined && value.fit !== 'cover' && value.fit !== 'contain') {
      error('backgroundMedia.fit', 'INVALID_BACKGROUND_MEDIA_FIT', 'Background media fit must be cover or contain');
    }
    if (value.position !== undefined && !['center', 'top', 'right', 'bottom', 'left'].includes(String(value.position))) {
      error('backgroundMedia.position', 'INVALID_BACKGROUND_MEDIA_POSITION', 'Background media position is invalid');
    }
  }

  private validateVideoAssetUsage(
    backgroundMedia: unknown,
    assets: unknown,
    requiredCapabilities: unknown,
    error: (path: string, code: string, message: string) => void,
  ): void {
    const videoAssetIds = isRecord(assets)
      ? Object.entries(assets)
        .filter(([, asset]) => isRecord(asset) && asset.kind === 'video')
        .map(([id]) => id)
      : [];
    if (backgroundMedia !== undefined
      && (!Array.isArray(requiredCapabilities) || !requiredCapabilities.includes('background-media.v1'))) {
      error(
        'requiredCapabilities',
        'MISSING_BACKGROUND_MEDIA_CAPABILITY',
        'Background media requires the background-media.v1 capability',
      );
    }
    const selectedVideoId = isRecord(backgroundMedia) && typeof backgroundMedia.assetId === 'string'
      ? backgroundMedia.assetId
      : undefined;
    videoAssetIds.forEach(assetId => {
      if (assetId !== selectedVideoId) {
        error(
          `assets.${assetId}`,
          'UNUSED_VIDEO_ASSET',
          'Video assets are allowed only when referenced by top-level backgroundMedia',
        );
      }
    });
  }

  private validateId(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (typeof value !== 'string' || value.length > 100 || !ID_PATTERN.test(value)) {
      error(path, 'INVALID_ID', 'Id must be a lowercase dotted or dashed identifier');
    }
  }

  private validateGlobals(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value)) {
      error(path, 'INVALID_GLOBALS', 'Globals must be an object');
      return;
    }
    const validators: Record<string, (value: unknown, path: string, error: (path: string, code: string, message: string) => void) => void> = {
      colors: this.validateColor.bind(this),
      lengths: this.validateLength.bind(this),
      numbers: this.validateNumber.bind(this),
      durations: this.validateDuration.bind(this),
      easings: this.validateEasing.bind(this),
      shadows: this.validateShadow.bind(this),
    };
    Object.entries(value).forEach(([group, entries]) => {
      const validator = validators[group];
      if (!validator) {
        error(`${path}.${group}`, 'UNKNOWN_GLOBAL_GROUP', `Unknown global token group: ${group}`);
        return;
      }
      if (!isRecord(entries)) {
        error(`${path}.${group}`, 'INVALID_TOKEN_GROUP', 'Token group must be an object');
        return;
      }
      Object.entries(entries).forEach(([id, token]) => {
        this.validateId(id, `${path}.${group}.${id}`, error);
        validator(token, `${path}.${group}.${id}`, error);
      });
    });
  }

  private validateMaterials(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
    warning: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value)) {
      error(path, 'INVALID_MATERIALS', 'Materials must be an object');
      return;
    }
    Object.entries(value).forEach(([id, definition]) => {
      this.validateId(id, `${path}.${id}`, error);
      if (!isRecord(definition) || !isRecord(definition.style)) {
        error(`${path}.${id}`, 'INVALID_MATERIAL', 'Material must contain a style object');
        return;
      }
      this.validateKnownKeys(definition, ['style', 'visualRole'], `${path}.${id}`, error);
      if (definition.visualRole !== undefined
        && (typeof definition.visualRole !== 'string' || !VISUAL_ROLES.has(definition.visualRole))) {
        error(`${path}.${id}.visualRole`, 'INVALID_VISUAL_ROLE', `Unknown visual role ${String(definition.visualRole)}`);
      }
      this.validateStyle(definition.style, `${path}.${id}.style`, undefined, error, warning);
    });
  }

  private validateSurfaces(
    value: unknown,
    path: string,
    getDescriptor: (id: string) => AppearanceSurfaceDescriptor | undefined,
    materials: unknown,
    error: ValidationErrorReporter,
    warning: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value)) {
      error(path, 'INVALID_SURFACES', 'Surface definitions must be an object');
      return;
    }
    Object.entries(value).forEach(([id, surface]) => {
      const descriptor = getDescriptor(id);
      if (!descriptor) {
        error(
          `${path}.${id}`,
          'UNKNOWN_SURFACE',
          `No registered appearance contract for ${id}`,
          {
            surfaceKind: path === 'components' ? 'component' : 'scene',
            surfaceId: id,
          },
        );
        return;
      }
      this.validateSurface(surface, `${path}.${id}`, descriptor, materials, error, warning);
    });
  }

  private validateSurface(
    value: unknown,
    path: string,
    descriptor: AppearanceSurfaceDescriptor,
    materials: unknown,
    error: ValidationErrorReporter,
    warning: (path: string, code: string, message: string) => void,
  ): void {
    if (!isRecord(value) || !isRecord(value.parts)) {
      error(path, 'INVALID_SURFACE', 'Surface must contain a parts object');
      return;
    }
    this.validateKnownKeys(value, ['parts'], path, error);
    const parts = new Map(descriptor.parts.map(part => [part.id, part]));
    const facets = new Map((descriptor.facets ?? []).map(facet => [facet.id, facet]));
    const states = new Set((descriptor.states ?? []).map(state => state.id));
    Object.entries(value.parts).forEach(([partId, rawRule]) => {
      const part = parts.get(partId);
      const partPath = `${path}.parts.${partId}`;
      if (!part) {
        error(partPath, 'UNKNOWN_PART', `Unknown part ${partId}`, {
          surfaceKind: path.startsWith('components.') ? 'component' : 'scene',
          surfaceId: descriptor.id,
          partId,
          allowedParts: descriptor.parts.map(candidate => candidate.id),
        });
        return;
      }
      if (!isRecord(rawRule)) {
        error(partPath, 'INVALID_PART_RULE', 'Part rule must be an object');
        return;
      }
      this.validateKnownKeys(
        rawRule,
        ['cascade', 'materials', 'decorationIntent', 'base', 'facets', 'states', 'contexts'],
        partPath,
        error,
      );
      if (rawRule.cascade !== undefined && rawRule.cascade !== 'normal' && rawRule.cascade !== 'override') {
        error(`${partPath}.cascade`, 'INVALID_CASCADE', 'Cascade must be normal or override');
      }
      if (rawRule.decorationIntent !== undefined
        && !['flat', 'separator', 'framed'].includes(String(rawRule.decorationIntent))) {
        error(`${partPath}.decorationIntent`, 'INVALID_DECORATION_INTENT', 'Decoration intent must be flat, separator, or framed');
      }
      const allowedProperties = part.allowedProperties
        ?? getAppearanceProfileProperties(part.propertyProfile);
      const materialStyles: Record<string, unknown>[] = [];
      if (rawRule.materials !== undefined) {
        if (!Array.isArray(rawRule.materials) || rawRule.materials.length === 0 || rawRule.materials.length > 8) {
          error(`${partPath}.materials`, 'INVALID_MATERIAL_LIST', 'Materials must contain between 1 and 8 material ids');
        } else {
          rawRule.materials.forEach((materialId, index) => {
            if (typeof materialId !== 'string' || !isRecord(materials) || !(materialId in materials)) {
              error(`${partPath}.materials.${index}`, 'UNKNOWN_MATERIAL', `Unknown material ${String(materialId)}`);
              return;
            }
            const definition = materials[materialId];
            if (isRecord(definition) && isRecord(definition.style)) {
              materialStyles.push(definition.style);
              this.validateStyle(
                definition.style,
                `${partPath}.materials.${index}`,
                allowedProperties,
                error,
                warning,
              );
            }
          });
        }
      }
      this.validateStyle(rawRule.base, `${partPath}.base`, allowedProperties, error, warning);
      const effectiveBase = Object.assign({}, ...materialStyles, isRecord(rawRule.base) ? rawRule.base : {});
      this.auditPartVisualSemantics(effectiveBase, rawRule, part, partPath, warning);
      if (rawRule.facets !== undefined) {
        if (!isRecord(rawRule.facets)) {
          error(`${partPath}.facets`, 'INVALID_FACETS', 'Facets must be an object');
        } else {
          Object.entries(rawRule.facets).forEach(([facetId, options]) => {
            const facet = facets.get(facetId);
            if (!facet) {
              error(`${partPath}.facets.${facetId}`, 'UNKNOWN_FACET', `Unknown facet ${facetId}`);
              return;
            }
            if (!isRecord(options)) {
              error(`${partPath}.facets.${facetId}`, 'INVALID_FACET_OPTIONS', 'Facet options must be an object');
              return;
            }
            Object.entries(options).forEach(([option, style]) => {
              const stylePath = `${partPath}.facets.${facetId}.${option}`;
              if (!facet.values.includes(option)) {
                error(stylePath, 'UNKNOWN_FACET_VALUE', `Unknown ${facetId} value ${option}`);
                return;
              }
              this.validateStyle(style, stylePath, allowedProperties, error, warning);
              if (isRecord(style)) this.auditPartVisualSemantics(style, rawRule, part, stylePath, warning);
            });
          });
        }
      }
      if (rawRule.states !== undefined) {
        if (!isRecord(rawRule.states)) {
          error(`${partPath}.states`, 'INVALID_STATES', 'States must be an object');
        } else {
          Object.entries(rawRule.states).forEach(([stateId, style]) => {
            const stylePath = `${partPath}.states.${stateId}`;
            if (!states.has(stateId)) {
              error(stylePath, 'UNKNOWN_STATE', `Unknown state ${stateId}`);
              return;
            }
            this.validateStyle(style, stylePath, allowedProperties, error, warning);
            if (isRecord(style)) this.auditPartVisualSemantics(style, rawRule, part, stylePath, warning);
          });
        }
      }
      if (rawRule.contexts !== undefined) {
        if (!Array.isArray(rawRule.contexts)) {
          error(`${partPath}.contexts`, 'INVALID_CONTEXTS', 'Contexts must be an array');
        } else {
          rawRule.contexts.forEach((context, index) => {
            const contextPath = `${partPath}.contexts.${index}`;
            if (!isRecord(context) || !isRecord(context.when)) {
              error(contextPath, 'INVALID_CONTEXT', 'Context must contain a when object');
              return;
            }
            this.validateKnownKeys(context, ['when', 'style'], contextPath, error);
            this.validateKnownKeys(context.when, ['facets', 'states'], `${contextPath}.when`, error);
            this.validateCondition(context.when, contextPath, facets, states, error);
            this.validateStyle(context.style, `${contextPath}.style`, allowedProperties, error, warning);
            if (isRecord(context.style)) {
              this.auditPartVisualSemantics(context.style, rawRule, part, `${contextPath}.style`, warning);
            }
          });
        }
      }
    });
  }

  private validateCondition(
    value: Record<string, unknown>,
    path: string,
    facets: Map<string, NonNullable<AppearanceSurfaceDescriptor['facets']>[number]>,
    states: Set<string>,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (value.facets !== undefined) {
      if (!isRecord(value.facets)) {
        error(`${path}.when.facets`, 'INVALID_CONDITION_FACETS', 'Condition facets must be an object');
      } else {
        Object.entries(value.facets).forEach(([facetId, option]) => {
          const facet = facets.get(facetId);
          if (!facet || typeof option !== 'string' || !facet.values.includes(option)) {
            error(`${path}.when.facets.${facetId}`, 'INVALID_CONDITION_FACET', `Invalid facet condition ${facetId}=${String(option)}`);
          }
        });
      }
    }
    if (value.states !== undefined) {
      if (!Array.isArray(value.states)) {
        error(`${path}.when.states`, 'INVALID_CONDITION_STATES', 'Condition states must be an array');
      } else {
        value.states.forEach((state, index) => {
          if (typeof state !== 'string' || !states.has(state)) {
            error(`${path}.when.states.${index}`, 'INVALID_CONDITION_STATE', `Invalid state condition ${String(state)}`);
          }
        });
      }
    }
  }

  private validateStyle(
    value: unknown,
    path: string,
    allowedProperties: readonly AppearanceStyleProperty[] | undefined,
    error: (path: string, code: string, message: string) => void,
    warning?: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value)) {
      error(path, 'INVALID_STYLE', 'Style must be an object');
      return;
    }
    const allowed = allowedProperties ? new Set(allowedProperties) : STYLE_PROPERTIES;
    this.validateBackgroundLayerGroup(value, path, error);
    if (value.borderStyle !== undefined && value.borderWidth === undefined) {
      warning?.(
        path,
        'BORDER_WIDTH_NORMALIZED',
        'Border style without a global width is normalized to zero before side widths are applied',
      );
    }
    if ((value.outlineWidth !== undefined || value.outlineColor !== undefined)
      && value.outlineStyle === undefined) {
      warning?.(
        path,
        'OUTLINE_STYLE_NORMALIZED',
        'Outline width/color without a style is normalized to solid',
      );
    }
    Object.entries(value).forEach(([rawProperty, propertyValue]) => {
      const property = rawProperty as AppearanceStyleProperty;
      const propertyPath = `${path}.${rawProperty}`;
      if (!STYLE_PROPERTIES.has(property) || !allowed.has(property)) {
        error(propertyPath, 'UNSUPPORTED_STYLE_PROPERTY', `Style property ${rawProperty} is not allowed`);
        return;
      }
      if (COLOR_PROPERTIES.has(property)) {
        this.validateColor(propertyValue, propertyPath, error);
      } else if (LENGTH_PROPERTIES.has(property)) {
        this.validateLength(propertyValue, propertyPath, error);
      } else if (NUMBER_PROPERTIES.has(property)) {
        this.validateNumber(propertyValue, propertyPath, error);
      } else if (property === 'backgroundImage') {
        this.validateAssetReference(propertyValue, propertyPath, error);
      } else if (property === 'backgroundImages') {
        if (!Array.isArray(propertyValue) || propertyValue.length === 0 || propertyValue.length > 8) {
          error(propertyPath, 'INVALID_BACKGROUND_IMAGES', 'Background images must contain between 1 and 8 package asset references');
        } else {
          propertyValue.forEach((asset, index) => this.validateAssetReference(asset, `${propertyPath}.${index}`, error));
        }
      } else if (property === 'backgroundSizes') {
        if (!Array.isArray(propertyValue)) {
          error(propertyPath, 'INVALID_BACKGROUND_SIZES', 'Background sizes must be an array');
        } else {
          propertyValue.forEach((size, index) => this.validateBackgroundSize(size, `${propertyPath}.${index}`, error));
        }
      } else if (property === 'backgroundPositions') {
        if (!Array.isArray(propertyValue)) {
          error(propertyPath, 'INVALID_BACKGROUND_POSITIONS', 'Background positions must be an array');
        } else {
          propertyValue.forEach((position, index) => this.validateBackgroundPosition(position, `${propertyPath}.${index}`, error));
        }
      } else if (property === 'backgroundRepeats') {
        this.validateStringArray(propertyValue, propertyPath, BACKGROUND_REPEAT_VALUES, 'INVALID_BACKGROUND_REPEATS', error);
      } else if (property === 'backgroundBlendModes') {
        this.validateStringArray(propertyValue, propertyPath, BACKGROUND_BLEND_MODE_VALUES, 'INVALID_BACKGROUND_BLEND_MODES', error);
      } else if (property === 'boxShadow') {
        if (!Array.isArray(propertyValue)) {
          error(propertyPath, 'INVALID_SHADOW_LIST', 'Box shadow must be an array');
        } else {
          propertyValue.forEach((shadow, index) => this.validateShadow(shadow, `${propertyPath}.${index}`, error));
        }
      } else if (property === 'transition') {
        this.validateTransition(propertyValue, propertyPath, error);
      } else if (property === 'transform') {
        this.validateTransform(propertyValue, propertyPath, error);
      } else if (property === 'gridTemplateColumns' || property === 'gridTemplateRows') {
        this.validateGridTemplate(propertyValue, propertyPath, error);
      } else if (property === 'aspectRatio') {
        this.validateRatio(propertyValue, propertyPath, error);
      } else if (property === 'filter') {
        this.validateFilter(propertyValue, propertyPath, error);
      } else {
        const values = ENUM_VALUES[property];
        if (!values?.has(String(propertyValue))) {
          error(propertyPath, 'INVALID_ENUM_VALUE', `Invalid value for ${property}`);
        }
      }
    });
  }

  private validateBackgroundLayerGroup(
    style: Record<string, unknown>,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    const layeredKeys = [
      'backgroundSizes', 'backgroundPositions', 'backgroundRepeats', 'backgroundBlendModes',
    ] as const;
    const singularKeys = ['backgroundImage', 'backgroundSize', 'backgroundPosition', 'backgroundRepeat'] as const;
    const images = style.backgroundImages;

    if (images === undefined) {
      layeredKeys.forEach(key => {
        if (style[key] !== undefined) {
          error(`${path}.${key}`, 'BACKGROUND_LAYERS_REQUIRE_IMAGES', `${key} requires backgroundImages`);
        }
      });
      return;
    }

    singularKeys.forEach(key => {
      if (style[key] !== undefined) {
        error(`${path}.${key}`, 'CONFLICTING_BACKGROUND_FIELDS', `${key} cannot be combined with backgroundImages`);
      }
    });

    if (!Array.isArray(images)) return;
    layeredKeys.forEach(key => {
      const values = style[key];
      if (Array.isArray(values) && values.length !== images.length) {
        error(`${path}.${key}`, 'BACKGROUND_LAYER_COUNT_MISMATCH', `${key} must contain exactly ${images.length} entries`);
      }
    });
  }

  private validateAssetReference(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (!isRecord(value) || value.kind !== 'asset' || typeof value.assetId !== 'string' || !ID_PATTERN.test(value.assetId)) {
      error(path, 'INVALID_ASSET_REFERENCE', 'Background image must be a package asset reference');
    }
  }

  private validateBackgroundSize(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (typeof value === 'string' && BACKGROUND_SIZE_VALUES.has(value)) return;
    if (!isRecord(value) || value.kind !== 'backgroundSize') {
      error(path, 'INVALID_BACKGROUND_SIZE', 'Background size must be auto, cover, contain, or a structured size');
      return;
    }
    this.validateKnownKeys(value, ['kind', 'width', 'height'], path, error);
    this.validateBackgroundDimension(value.width, `${path}.width`, error);
    if (value.height !== undefined) this.validateBackgroundDimension(value.height, `${path}.height`, error);
  }

  private validateBackgroundPosition(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (typeof value === 'string' && BACKGROUND_POSITION_VALUES.has(value)) return;
    if (!isRecord(value) || value.kind !== 'backgroundPosition') {
      error(path, 'INVALID_BACKGROUND_POSITION', 'Background position must be a keyword or structured x/y position');
      return;
    }
    this.validateKnownKeys(value, ['kind', 'x', 'y'], path, error);
    this.validateBackgroundPositionCoordinate(value.x, `${path}.x`, error);
    this.validateBackgroundPositionCoordinate(value.y, `${path}.y`, error);
  }

  private validateBackgroundDimension(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (this.validateReference(value, 'lengths', path, error)) return;
    if (isRecord(value) && value.kind === 'zero') return;
    if (isRecord(value)
      && ['px', 'rem', 'em', 'ch', 'vw', 'vh', 'percent'].includes(String(value.kind))
      && finiteNumber(value.value, 0, 10000)) return;
    if (isRecord(value) && value.kind === 'lengthKeyword' && value.value === 'auto') return;
    error(path, 'INVALID_BACKGROUND_DIMENSION', 'Background dimensions must be non-negative lengths, auto, or references');
  }

  private validateBackgroundPositionCoordinate(
    value: unknown,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (this.validateReference(value, 'lengths', path, error)) return;
    if (isRecord(value) && value.kind === 'zero') return;
    if (isRecord(value)
      && ['px', 'rem', 'em', 'ch', 'vw', 'vh', 'percent'].includes(String(value.kind))
      && finiteNumber(value.value, -10000, 10000)) return;
    error(path, 'INVALID_BACKGROUND_POSITION_COORDINATE', 'Background position coordinates must be lengths or references');
  }

  private validateStringArray(
    value: unknown,
    path: string,
    allowed: ReadonlySet<string>,
    code: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (!Array.isArray(value)) {
      error(path, code, 'Value must be an array');
      return;
    }
    value.forEach((item, index) => {
      if (typeof item !== 'string' || !allowed.has(item)) {
        error(`${path}.${index}`, code, `Unsupported value ${String(item)}`);
      }
    });
  }

  private validateReference(
    value: unknown,
    expectedGroup: AppearanceTokenGroup,
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): boolean {
    if (!isRecord(value) || value.kind !== 'ref' || typeof value.path !== 'string') return false;
    if (!REFERENCE_PATTERN.test(value.path)) return false;
    if (!value.path.startsWith(`globals.${expectedGroup}.`)) {
      error(
        path,
        'REFERENCE_TYPE_MISMATCH',
        `Reference must target globals.${expectedGroup}`,
      );
    }
    return true;
  }

  private validateColor(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (this.validateReference(value, 'colors', path, error)) return;
    if (!isRecord(value)) {
      error(path, 'INVALID_COLOR', 'Color must be a structured color value');
      return;
    }
    if (value.kind === 'transparent') return;
    if (value.kind === 'hex' && typeof value.value === 'string' && /^#[0-9a-f]{3,8}$/i.test(value.value) && [4, 5, 7, 9].includes(value.value.length)) return;
    if (value.kind === 'rgb' && finiteNumber(value.r, 0, 255) && finiteNumber(value.g, 0, 255) && finiteNumber(value.b, 0, 255) && (value.a === undefined || finiteNumber(value.a, 0, 1))) return;
    if (value.kind === 'hsl' && finiteNumber(value.h, 0, 360) && finiteNumber(value.s, 0, 100) && finiteNumber(value.l, 0, 100) && (value.a === undefined || finiteNumber(value.a, 0, 1))) return;
    error(path, 'INVALID_COLOR', 'Invalid structured color value');
  }

  private validateLength(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (this.validateReference(value, 'lengths', path, error)) return;
    if (isRecord(value) && value.kind === 'zero') return;
    if (isRecord(value) && ['px', 'rem', 'em', 'ch', 'vw', 'vh', 'percent'].includes(String(value.kind)) && finiteNumber(value.value, -10000, 10000)) return;
    if (isRecord(value) && value.kind === 'lengthKeyword' && ['auto', 'min-content', 'max-content', 'fit-content'].includes(String(value.value))) return;
    error(path, 'INVALID_LENGTH', 'Length must be a structured unit, keyword, zero, or reference value');
  }

  private validateNumber(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (this.validateReference(value, 'numbers', path, error)) return;
    if (isRecord(value) && value.kind === 'number' && finiteNumber(value.value, -10000, 10000)) return;
    error(path, 'INVALID_NUMBER', 'Number must be a structured number or reference value');
  }

  private validateDuration(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (this.validateReference(value, 'durations', path, error)) return;
    if (isRecord(value) && value.kind === 'ms' && finiteNumber(value.value, 0, 10000)) return;
    error(path, 'INVALID_DURATION', 'Duration must be a structured millisecond or reference value');
  }

  private validateEasing(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (this.validateReference(value, 'easings', path, error)) return;
    if (isRecord(value) && value.kind === 'easing' && ['linear', 'standard', 'decelerate', 'accelerate'].includes(String(value.value))) return;
    error(path, 'INVALID_EASING', 'Easing must be a supported structured easing value');
  }

  private validateShadow(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (this.validateReference(value, 'shadows', path, error)) return;
    if (isRecord(value) && value.kind === 'none') return;
    if (!isRecord(value) || value.kind !== 'shadow') {
      error(path, 'INVALID_SHADOW', 'Shadow must be a structured shadow, none, or reference value');
      return;
    }
    this.validateLength(value.x, `${path}.x`, error);
    this.validateLength(value.y, `${path}.y`, error);
    this.validateLength(value.blur, `${path}.blur`, error);
    if (value.spread !== undefined) this.validateLength(value.spread, `${path}.spread`, error);
    this.validateColor(value.color, `${path}.color`, error);
    if (value.inset !== undefined && typeof value.inset !== 'boolean') {
      error(`${path}.inset`, 'INVALID_SHADOW_INSET', 'Shadow inset must be boolean');
    }
  }

  private validateTransition(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (!Array.isArray(value) || value.length > 12) {
      error(path, 'INVALID_TRANSITION', 'Transition must be an array with at most 12 items');
      return;
    }
    const properties = new Set([
      'all', 'background-color', 'background-position', 'border-color', 'border-radius',
      'box-shadow', 'color', 'filter', 'gap', 'height', 'margin', 'opacity', 'padding',
      'width', 'transform',
    ]);
    value.forEach((item, index) => {
      const itemPath = `${path}.${index}`;
      if (!isRecord(item) || !properties.has(String(item.property))) {
        error(itemPath, 'INVALID_TRANSITION_ITEM', 'Transition item has an unsupported property');
        return;
      }
      this.validateDuration(item.duration, `${itemPath}.duration`, error);
      this.validateEasing(item.easing, `${itemPath}.easing`, error);
      if (item.delay !== undefined) this.validateDuration(item.delay, `${itemPath}.delay`, error);
    });
  }

  private validateTransform(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (!isRecord(value) || value.kind !== 'transform') {
      error(path, 'INVALID_TRANSFORM', 'Transform must be a structured transform value');
      return;
    }
    if (value.scale !== undefined && !finiteNumber(value.scale, 0, 10)) {
      error(`${path}.scale`, 'INVALID_SCALE', 'Scale must be between 0 and 10');
    }
    if (value.translateX !== undefined) this.validateLength(value.translateX, `${path}.translateX`, error);
    if (value.translateY !== undefined) this.validateLength(value.translateY, `${path}.translateY`, error);
    if (value.rotate !== undefined && !finiteNumber(value.rotate, -3600, 3600)) {
      error(`${path}.rotate`, 'INVALID_ROTATION', 'Rotation must be between -3600 and 3600 degrees');
    }
  }

  private validateGridTemplate(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (!isRecord(value) || value.kind !== 'gridTracks' || !Array.isArray(value.tracks) || value.tracks.length === 0 || value.tracks.length > 24) {
      error(path, 'INVALID_GRID_TEMPLATE', 'Grid template must contain between 1 and 24 structured tracks');
      return;
    }
    value.tracks.forEach((track, index) => {
      const trackPath = `${path}.tracks.${index}`;
      if (isRecord(track) && track.kind === 'fr' && finiteNumber(track.value, 0.01, 100)) return;
      if (isRecord(track) && track.kind === 'trackKeyword' && ['auto', 'min-content', 'max-content'].includes(String(track.value))) return;
      this.validateLength(track, trackPath, error);
    });
  }

  private validateRatio(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (!isRecord(value)
      || value.kind !== 'ratio'
      || !finiteNumber(value.width, 0.01, 10000)
      || !finiteNumber(value.height, 0.01, 10000)) {
      error(path, 'INVALID_ASPECT_RATIO', 'Aspect ratio must contain positive width and height values');
    }
  }

  private validateFilter(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (!Array.isArray(value) || value.length > 8) {
      error(path, 'INVALID_FILTER', 'Filter must be an array with at most 8 items');
      return;
    }
    value.forEach((item, index) => {
      const itemPath = `${path}.${index}`;
      if (!isRecord(item)) {
        error(itemPath, 'INVALID_FILTER_ITEM', 'Filter item must be structured');
        return;
      }
      if (item.kind === 'blur') {
        this.validateLength(item.value, `${itemPath}.value`, error);
        return;
      }
      if (['brightness', 'contrast', 'grayscale', 'saturate'].includes(String(item.kind))) {
        this.validateNumber(item.value, `${itemPath}.value`, error);
        return;
      }
      if (item.kind === 'hueRotate' && finiteNumber(item.degrees, -3600, 3600)) return;
      error(itemPath, 'INVALID_FILTER_ITEM', `Unsupported filter ${String(item.kind)}`);
    });
  }

  private validateRenderers(
    value: unknown,
    registry: AppearanceRegistry,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value)) {
      error('renderers', 'INVALID_RENDERERS', 'Renderers must be an object');
      return;
    }
    Object.entries(value).forEach(([id, definition]) => {
      const adapter = registry.getRenderer(id);
      const path = `renderers.${id}`;
      if (!adapter) {
        error(path, 'UNKNOWN_RENDERER', `No registered renderer adapter for ${id}`);
        return;
      }
      if (!isRecord(definition) || definition.version !== 1 || !isRecord(definition.settings)) {
        error(path, 'INVALID_RENDERER_DEFINITION', 'Renderer definition must use version 1 and contain settings');
        return;
      }
      this.validateKnownKeys(definition, ['version', 'settings'], path, error);
      this.scanRendererSettings(definition.settings, `${path}.settings`, error);
      adapter.validate(definition.settings).forEach(message => {
        error(`${path}.settings`, 'INVALID_RENDERER_SETTINGS', message);
      });
    });
  }

  private scanRendererSettings(value: unknown, path: string, error: (path: string, code: string, message: string) => void): void {
    if (typeof value === 'string') {
      if (FORBIDDEN_TEXT_PATTERN.test(value)) {
        error(path, 'FORBIDDEN_RENDERER_TEXT', 'Renderer settings cannot contain URLs, markup, or CSS syntax');
      }
      return;
    }
    if (Array.isArray(value)) {
      value.forEach((item, index) => this.scanRendererSettings(item, `${path}.${index}`, error));
      return;
    }
    if (isRecord(value)) {
      Object.entries(value).forEach(([key, item]) => this.scanRendererSettings(item, `${path}.${key}`, error));
    }
  }

  private validateAssets(value: unknown, error: (path: string, code: string, message: string) => void): void {
    if (value === undefined) return;
    if (!isRecord(value)) {
      error('assets', 'INVALID_ASSETS', 'Assets must be an object');
      return;
    }
    Object.entries(value).forEach(([id, asset]) => {
      const path = `assets.${id}`;
      this.validateId(id, path, error);
      const validImage = isRecord(asset)
        && asset.kind === 'image'
        && ['image/png', 'image/jpeg', 'image/webp', 'image/gif'].includes(String(asset.mimeType));
      const validVideo = isRecord(asset)
        && asset.kind === 'video'
        && ['video/mp4', 'video/webm'].includes(String(asset.mimeType));
      if (!validImage && !validVideo) {
        error(path, 'INVALID_ASSET', 'Asset must be a supported image or video');
        return;
      }
      this.validateKnownKeys(asset, ['kind', 'mimeType', 'source'], path, error);
      const source = asset.source;
      if (!isRecord(source) || source.kind !== 'package' || typeof source.path !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9_./-]*$/.test(source.path) || source.path.includes('..') || source.path.startsWith('/')) {
        error(`${path}.source`, 'INVALID_ASSET_PATH', 'Asset path must be a safe package-relative path');
      } else {
        this.validateKnownKeys(source, ['kind', 'path'], `${path}.source`, error);
      }
    });
  }

  private validateIntegrity(
    value: unknown,
    assets: unknown,
    error: (path: string, code: string, message: string) => void,
  ): void {
    if (value === undefined) return;
    if (!isRecord(value) || !isRecord(value.sha256)) {
      error('integrity', 'INVALID_INTEGRITY', 'Integrity must contain a sha256 object');
      return;
    }
    Object.entries(value.sha256).forEach(([assetId, digest]) => {
      if (!isRecord(assets) || !(assetId in assets)) {
        error(`integrity.sha256.${assetId}`, 'UNKNOWN_INTEGRITY_ASSET', `Unknown integrity asset ${assetId}`);
      }
      if (typeof digest !== 'string' || !/^[0-9a-f]{64}$/i.test(digest)) {
        error(`integrity.sha256.${assetId}`, 'INVALID_SHA256', 'SHA-256 digest must contain 64 hexadecimal characters');
      }
    });
  }

  private validateAssetReferences(
    materials: unknown,
    components: unknown,
    scenes: unknown,
    assets: unknown,
    error: (path: string, code: string, message: string) => void,
  ): void {
    const declaredAssets = new Set(isRecord(assets) ? Object.keys(assets) : []);
    const validateStyle = (style: unknown, path: string): void => {
      if (!isRecord(style)) return;
      const references: Array<{ value: unknown; path: string }> = [];
      if (style.backgroundImage !== undefined) {
        references.push({ value: style.backgroundImage, path: `${path}.backgroundImage` });
      }
      if (Array.isArray(style.backgroundImages)) {
        style.backgroundImages.forEach((value, index) => {
          references.push({ value, path: `${path}.backgroundImages.${index}` });
        });
      }
      references.forEach(reference => {
        if (!isRecord(reference.value)
          || reference.value.kind !== 'asset'
          || typeof reference.value.assetId !== 'string') return;
        const assetId = reference.value.assetId;
        if (!declaredAssets.has(assetId)) {
          error(reference.path, 'UNKNOWN_ASSET_REFERENCE', `Unknown appearance asset ${assetId}`);
          return;
        }
        const referencedAsset = isRecord(assets) ? assets[assetId] : undefined;
        if (isRecord(referencedAsset) && referencedAsset.kind !== 'image') {
          error(reference.path, 'BACKGROUND_ASSET_NOT_IMAGE', 'Style background assets must be images');
        }
      });
    };
    const validateSurfaces = (surfaces: unknown, path: string): void => {
      if (!isRecord(surfaces)) return;
      Object.entries(surfaces).forEach(([surfaceId, surface]) => {
        if (!isRecord(surface) || !isRecord(surface.parts)) return;
        Object.entries(surface.parts).forEach(([partId, rule]) => {
          if (!isRecord(rule)) return;
          const partPath = `${path}.${surfaceId}.parts.${partId}`;
          validateStyle(rule.base, `${partPath}.base`);
          if (isRecord(rule.facets)) {
            Object.entries(rule.facets).forEach(([facetId, options]) => {
              if (!isRecord(options)) return;
              Object.entries(options).forEach(([option, style]) => {
                validateStyle(style, `${partPath}.facets.${facetId}.${option}`);
              });
            });
          }
          if (isRecord(rule.states)) {
            Object.entries(rule.states).forEach(([stateId, style]) => {
              validateStyle(style, `${partPath}.states.${stateId}`);
            });
          }
          if (Array.isArray(rule.contexts)) {
            rule.contexts.forEach((context, index) => {
              if (isRecord(context)) validateStyle(context.style, `${partPath}.contexts.${index}.style`);
            });
          }
        });
      });
    };

    if (isRecord(materials)) {
      Object.entries(materials).forEach(([materialId, definition]) => {
        if (isRecord(definition)) {
          validateStyle(definition.style, `materials.${materialId}.style`);
        }
      });
    }
    validateSurfaces(components, 'components');
    validateSurfaces(scenes, 'scenes');
  }

  private validateReferenceGraph(
    pkg: Record<string, unknown>,
    error: (path: string, code: string, message: string) => void,
  ): void {
    const globals = isRecord(pkg.globals) ? pkg.globals : {};
    const tokenPaths = new Set<string>();
    const references: Array<{ sourcePath: string; targetPath: string }> = [];
    const graph = new Map<string, Set<string>>();

    Object.entries(globals).forEach(([group, entries]) => {
      if (!isRecord(entries)) return;
      Object.keys(entries).forEach(id => tokenPaths.add(`globals.${group}.${id}`));
    });

    const scan = (value: unknown, path: string, ownerToken?: string): void => {
      if (Array.isArray(value)) {
        value.forEach((item, index) => scan(item, `${path}.${index}`, ownerToken));
        return;
      }
      if (!isRecord(value)) return;
      if (value.kind === 'ref' && typeof value.path === 'string' && REFERENCE_PATTERN.test(value.path)) {
        references.push({ sourcePath: path, targetPath: value.path });
        if (ownerToken) {
          const targets = graph.get(ownerToken) ?? new Set<string>();
          targets.add(value.path);
          graph.set(ownerToken, targets);
        }
        return;
      }
      Object.entries(value).forEach(([key, child]) => {
        const childPath = path === '$' ? key : `${path}.${key}`;
        scan(child, childPath, ownerToken);
      });
    };

    Object.entries(globals).forEach(([group, entries]) => {
      if (!isRecord(entries)) return;
      Object.entries(entries).forEach(([id, value]) => {
        const tokenPath = `globals.${group}.${id}`;
        scan(value, tokenPath, tokenPath);
      });
    });
    Object.entries(pkg).forEach(([key, value]) => {
      if (key !== 'globals') scan(value, key);
    });

    references.forEach(reference => {
      if (!tokenPaths.has(reference.targetPath)) {
        error(
          reference.sourcePath,
          'UNKNOWN_TOKEN_REFERENCE',
          `Unknown appearance token reference: ${reference.targetPath}`,
        );
      }
    });

    const visited = new Set<string>();
    const visiting = new Set<string>();
    const visit = (tokenPath: string): void => {
      if (visited.has(tokenPath)) return;
      if (visiting.has(tokenPath)) {
        error(tokenPath, 'CIRCULAR_TOKEN_REFERENCE', `Circular appearance token reference: ${tokenPath}`);
        return;
      }
      visiting.add(tokenPath);
      graph.get(tokenPath)?.forEach(targetPath => {
        if (tokenPaths.has(targetPath)) visit(targetPath);
      });
      visiting.delete(tokenPath);
      visited.add(tokenPath);
    };
    tokenPaths.forEach(visit);
  }

  private auditPartVisualSemantics(
    style: Record<string, unknown>,
    rule: Record<string, unknown>,
    part: AppearanceSurfaceDescriptor['parts'][number],
    path: string,
    warning: (path: string, code: string, message: string) => void,
  ): void {
    if ((!part.continuityGroup && part.visualRole !== 'continuous-surface')
      || rule.decorationIntent === 'framed') return;
    const hasFullFrame = this.hasNonZeroLength(style.borderWidth)
      || [
        style.borderRadius,
        style.borderTopLeftRadius,
        style.borderTopRightRadius,
        style.borderBottomRightRadius,
        style.borderBottomLeftRadius,
      ].some(value => this.hasNonZeroLength(value))
      || (Array.isArray(style.boxShadow)
        && style.boxShadow.some(shadow => !isRecord(shadow) || shadow.kind !== 'none'));
    if (!hasFullFrame) return;
    warning(
      path,
      'CONTINUOUS_SURFACE_FRAMED',
      `Continuous surface${part.continuityGroup ? ` ${part.continuityGroup}` : ''} uses a full frame without decorationIntent=framed`,
    );
  }

  private hasNonZeroLength(value: unknown): boolean {
    if (!isRecord(value)) return value !== undefined;
    if (value.kind === 'zero') return false;
    return !(typeof value.value === 'number' && value.value === 0);
  }

  private auditCascadeUsage(
    components: unknown,
    scenes: unknown,
    warning: (path: string, code: string, message: string) => void,
  ): void {
    let total = 0;
    let overrides = 0;
    const inspect = (surfaces: unknown): void => {
      if (!isRecord(surfaces)) return;
      Object.values(surfaces).forEach(surface => {
        if (!isRecord(surface) || !isRecord(surface.parts)) return;
        Object.values(surface.parts).forEach(rule => {
          if (!isRecord(rule)) return;
          total += 1;
          if (rule.cascade === 'override') overrides += 1;
        });
      });
    };
    inspect(components);
    inspect(scenes);
    if (total >= 4 && overrides / total > 0.25) {
      warning(
        '$',
        'EXCESSIVE_OVERRIDE_USAGE',
        `${overrides} of ${total} part rules use override; override should be reserved for specific paint conflicts`,
      );
    }
  }

  private validateKnownKeys(
    value: Record<string, unknown>,
    allowedKeys: readonly string[],
    path: string,
    error: (path: string, code: string, message: string) => void,
  ): void {
    const allowed = new Set(allowedKeys);
    Object.keys(value).forEach(key => {
      if (!allowed.has(key)) {
        error(path === '$' ? key : `${path}.${key}`, 'UNKNOWN_FIELD', `Unknown field: ${key}`);
      }
    });
  }
}

export const appearancePackageValidator = new AppearancePackageValidator();

export function assertValidAppearancePackage(
  input: unknown,
  registry: AppearanceRegistry,
): AppearancePackage {
  const result = appearancePackageValidator.validate(input, registry);
  if (!result.valid || !result.value) {
    throw new AppearancePackageValidationError(result.errors);
  }
  return result.value;
}

export function isAppearanceSurfaceDefinition(value: unknown): value is AppearanceSurfaceDefinition {
  return isRecord(value) && isRecord(value.parts);
}

export function isAppearanceStyle(value: unknown): value is AppearanceStyle {
  return isRecord(value) && Object.keys(value).every(key => STYLE_PROPERTIES.has(key as AppearanceStyleProperty));
}
