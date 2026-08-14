interface GraphemeSegment {
  segment: string;
}

interface GraphemeSegmenter {
  segment: (text: string) => Iterable<GraphemeSegment>;
}

type GraphemeSegmenterConstructor = new (
  locale?: string | string[],
  options?: { granularity: 'grapheme' },
) => GraphemeSegmenter;

export function firstAvatarGrapheme(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return '';

  const Segmenter = (Intl as typeof Intl & {
    Segmenter?: GraphemeSegmenterConstructor;
  }).Segmenter;

  if (Segmenter) {
    const segmenter = new Segmenter(undefined, { granularity: 'grapheme' });
    const first = segmenter.segment(trimmed)[Symbol.iterator]().next();
    return first.done ? '' : first.value.segment;
  }

  return Array.from(trimmed)[0] ?? '';
}
