// Syntax highlight colors are renderer palettes, not app theme surface colors.
// Keep them centralized so Prism/Markdown surfaces do not carry app raw color debt.
const LIGHT_PRISM_PALETTE = {
  foreground: '#24292f',
  muted: '#6e7781',
  keyword: '#cf222e',
  literal: '#0550ae',
  functionName: '#8250df',
  tag: '#116329',
  punctuation: '#57606a',
  property: '#953800',
} as const;

const DARK_PRISM_PALETTE = {
  foreground: '#d4d4d4',
  comment: '#6a9955',
  keyword: '#c586c0',
  string: '#ce9178',
  functionName: '#dcdcaa',
  number: '#b5cea8',
  tag: '#569cd6',
  property: '#9cdcfe',
} as const;

export const SHARED_PRISM_COLOR_SCHEME = {
  light: {
    foreground: LIGHT_PRISM_PALETTE.foreground,
    comment: LIGHT_PRISM_PALETTE.muted,
    keyword: LIGHT_PRISM_PALETTE.keyword,
    string: LIGHT_PRISM_PALETTE.literal,
    functionName: LIGHT_PRISM_PALETTE.functionName,
    number: LIGHT_PRISM_PALETTE.literal,
    tag: LIGHT_PRISM_PALETTE.tag,
    punctuation: LIGHT_PRISM_PALETTE.punctuation,
    property: LIGHT_PRISM_PALETTE.property,
  },
  dark: {
    foreground: DARK_PRISM_PALETTE.foreground,
    comment: DARK_PRISM_PALETTE.comment,
    keyword: DARK_PRISM_PALETTE.keyword,
    string: DARK_PRISM_PALETTE.string,
    functionName: DARK_PRISM_PALETTE.functionName,
    number: DARK_PRISM_PALETTE.number,
    tag: DARK_PRISM_PALETTE.tag,
    punctuation: DARK_PRISM_PALETTE.foreground,
    property: DARK_PRISM_PALETTE.property,
  },
} as const;
