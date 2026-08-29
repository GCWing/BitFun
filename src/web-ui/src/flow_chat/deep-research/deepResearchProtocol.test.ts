import { describe, expect, it } from 'vitest';

import { parseDeepResearchContent } from './deepResearchProtocol';

describe('parseDeepResearchContent', () => {
  it('extracts a phase marker without leaving protocol text in markdown', () => {
    expect(parseDeepResearchContent('[[PHASE:phase-0-orient]]\n\nStarting research.')).toEqual({
      hasProtocol: true,
      segments: [
        {
          type: 'protocol',
          kind: 'phase',
          markers: [{
            kind: 'phase',
            phaseId: 'phase-0-orient',
            raw: '[[PHASE:phase-0-orient]]',
          }],
        },
        { type: 'markdown', content: 'Starting research.' },
      ],
    });
  });

  it('groups adjacent sub-question markers even when the model emits them on one line', () => {
    const parsed = parseDeepResearchContent(
      '[[SUBQ:q1|Market landscape|root]] [[SUBQ:q2|Fusion API progress|root]]\nResearch plan saved.',
    );

    expect(parsed.segments).toHaveLength(2);
    expect(parsed.segments[0]).toMatchObject({
      type: 'protocol',
      kind: 'subquestion',
      markers: [
        { id: 'q1', title: 'Market landscape', parentId: 'root' },
        { id: 'q2', title: 'Fusion API progress', parentId: 'root' },
      ],
    });
    expect(parsed.segments[1]).toEqual({ type: 'markdown', content: 'Research plan saved.' });
  });

  it('keeps pipes inside sub-question titles and citation URLs', () => {
    const parsed = parseDeepResearchContent(
      '[[SUBQ:q1|Routing | fusion trade-offs|root]]\n[[CITATION:cit_001|high|true|https://example.com/a|b]]',
    );

    expect(parsed.segments).toMatchObject([
      {
        type: 'protocol',
        markers: [{ title: 'Routing|fusion trade-offs' }],
      },
      {
        type: 'protocol',
        markers: [{ url: 'https://example.com/a|b' }],
      },
    ]);
  });

  it('parses verdict confidence as a bounded number', () => {
    const parsed = parseDeepResearchContent(
      '[[VERDICT:q1|DECIDED|0.87]]\n[[VERDICT:q2|CONTESTED|0.71]]',
    );

    expect(parsed.segments).toMatchObject([{
      type: 'protocol',
      kind: 'verdict',
      markers: [
        { subquestionId: 'q1', status: 'DECIDED', confidence: 0.87 },
        { subquestionId: 'q2', status: 'CONTESTED', confidence: 0.71 },
      ],
    }]);
  });

  it('preserves invalid markers as ordinary markdown', () => {
    const content = '[[PHASE:not-a-phase]] and [[VERDICT:q1|UNKNOWN|4]]';
    expect(parseDeepResearchContent(content)).toEqual({
      hasProtocol: false,
      segments: [{ type: 'markdown', content }],
    });
  });

  it('hides an incomplete protocol marker while streaming', () => {
    expect(parseDeepResearchContent('Preparing\n[[SUBQ:q1|Market')).toEqual({
      hasProtocol: true,
      segments: [{ type: 'markdown', content: 'Preparing\n' }],
    });
  });

  it('leaves unrelated double brackets untouched', () => {
    const content = 'Use [[wiki links]] in this note.';
    expect(parseDeepResearchContent(content)).toEqual({
      hasProtocol: false,
      segments: [{ type: 'markdown', content }],
    });
  });
});
