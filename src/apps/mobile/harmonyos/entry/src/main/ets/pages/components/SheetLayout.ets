import { MobileDesignGeometry } from '../../generated/MobileDesignTokens';

/**
 * HarmonyOS layout aliases for the cross-platform mobile Sheet contract.
 *
 * Keep only Harmony-specific constraints here. Shared geometry must continue
 * to come from mobile-tokens.json through MobileDesignGeometry.
 */
export const SHEET_CONTENT_MAX_WIDTH: number = 520;
export const SHEET_HORIZONTAL_PADDING: number = MobileDesignGeometry.sheetHorizontalPadding;
export const SHEET_HEADER_HEIGHT: number = MobileDesignGeometry.sheetHeaderHeight;
export const SHEET_ACTION_HEIGHT: number = MobileDesignGeometry.sheetActionHeight;
export const SHEET_TOP_RADIUS: number = MobileDesignGeometry.sheetTopRadius;
export const SHEET_SIDE_RADIUS: number = MobileDesignGeometry.sheetSideRadius;
export const SHEET_HEADER_SIDE_PADDING: number = 8;
export const SHEET_CONTENT_BOTTOM_PADDING: number = 18;
export const SHEET_FOOTER_TOP_PADDING: number = 10;
export const SHEET_FOOTER_BOTTOM_PADDING: number = 24;
