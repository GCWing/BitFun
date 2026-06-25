// Syntax highlight colors are renderer palettes, not app theme surface colors.
// Keep them centralized so Prism/Markdown surfaces do not carry app raw color debt.
export const SHARED_PRISM_COLOR_SCHEME = {
  light: {
    foreground: '#24292f',
    comment: '#6e7781',
    keyword: '#cf222e',
    string: '#0a3069',
    functionName: '#8250df',
    number: '#0550ae',
    tag: '#116329',
    punctuation: '#57606a',
    property: '#953800',
  },
  dark: {
    foreground: '#d4d4d4',
    comment: '#6a9955',
    keyword: '#c586c0',
    string: '#ce9178',
    functionName: '#dcdcaa',
    number: '#b5cea8',
    tag: '#569cd6',
    punctuation: '#d4d4d4',
    property: '#9cdcfe',
  },
} as const;
