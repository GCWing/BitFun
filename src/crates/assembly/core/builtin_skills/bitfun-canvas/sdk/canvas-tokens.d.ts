/**
 * Design-token contract for `bitfun/canvas`.
 *
 * Canvas uses host-provided BitFun semantic CSS variables instead of declaring
 * an independent hardcoded color palette. Treat exported colors as token
 * references or host-resolved strings; do not depend on concrete hex values.
 */
export interface CanvasPalette {
    readonly foreground: string;
    readonly foregroundSecondary: string;
    readonly foregroundTertiary: string;
    readonly foregroundQuaternary: string;
    readonly editor: string;
    readonly chrome: string;
    readonly sidebar: string;
    readonly elevated: string;
    readonly fillPrimary: string;
    readonly fillSecondary: string;
    readonly fillTertiary: string;
    readonly fillQuaternary: string;
    readonly strokePrimary: string;
    readonly strokeSecondary: string;
    readonly strokeTertiary: string;
    readonly strokeFocused: string;
    readonly accent: string;
    readonly buttonBackground: string;
    readonly buttonForeground: string;
    readonly buttonHoverBackground: string;
    readonly link: string;
    readonly diffInsertedLine: string;
    readonly diffRemovedLine: string;
    readonly diffStripAdded: string;
    readonly diffStripRemoved: string;
}
export interface CanvasHostThemeOverrides {
    readonly primary?: string;
    readonly editorBackground?: string;
    readonly editorForeground?: string;
}
export type Color = "gray" | "purple" | "green" | "yellow" | "cyan" | "pink" | "blue" | "orange";
export type CategoryPalette = Readonly<Record<Color, string>>;
export declare const canvasPaletteDark: CanvasPalette;
export declare const canvasPaletteLight: CanvasPalette;
export declare function applyWorkbenchSurfaces(palette: CanvasPalette, surfaces: Pick<CanvasHostThemeOverrides, "editorBackground" | "editorForeground">): CanvasPalette;
export declare function applyPrimaryColor(palette: CanvasPalette, primary: string): CanvasPalette;
export declare const categoryPaletteDark: CategoryPalette;
export declare const categoryPaletteLight: CategoryPalette;
/** Legacy `colorPalette` name kept for back-compat; prefer `useHostTheme().category`. */
export declare const colorPalette: CategoryPalette;
export declare const usageColorSequence: readonly Color[];
export declare const chartColorSequence: readonly string[];
export interface CanvasTokens {
    readonly bg: {
        readonly editor: string;
        readonly chrome: string;
        readonly elevated: string;
    };
    readonly text: {
        readonly primary: string;
        readonly secondary: string;
        readonly tertiary: string;
        readonly quaternary: string;
        readonly link: string;
        readonly onAccent: string;
    };
    readonly stroke: {
        readonly primary: string;
        readonly secondary: string;
        readonly tertiary: string;
        readonly focused: string;
    };
    readonly fill: {
        readonly primary: string;
        readonly secondary: string;
        readonly tertiary: string;
        readonly quaternary: string;
    };
    readonly accent: {
        readonly primary: string;
        readonly control: string;
        readonly controlHover: string;
    };
    readonly diff: {
        readonly insertedLine: string;
        readonly removedLine: string;
        readonly stripAdded: string;
        readonly stripRemoved: string;
    };
    readonly category: CategoryPalette;
}
/** Semantic colors for components. Spacing and radius live in `theme.ts`. */
export declare const canvasTokens: CanvasTokens;
export declare const canvasTokensLight: CanvasTokens;
export declare function buildHostTokens(kind: string, overrides?: CanvasHostThemeOverrides): {
    tokens: CanvasTokens;
    palette: CanvasPalette;
};
//# sourceMappingURL=canvas-tokens.d.ts.map
