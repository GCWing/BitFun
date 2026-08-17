import { afterEach, describe, expect, it } from 'vitest';
import { comparePositions } from './position';
import {
  resetSessionStreamsForTest,
  sessionStream,
} from './SessionStream';

afterEach(() => {
  resetSessionStreamsForTest();
});

function at(cursor: number, streamId = 'runtime-a') {
  return { streamId, cursor };
}

function textAt(turnId: string) {
  return { sessionId: 'session', turnId, text: 'x' };
}

const TEXT = 'agentic://text-chunk';
const STARTED = 'agentic://dialog-turn-started';
const COMPLETED = 'agentic://dialog-turn-completed';

describe('position ordering', () => {
  it('never orders cursors from different Runtime processes', () => {
    expect(comparePositions(at(9), at(3, 'runtime-b'))).toBe('unrelated');
    expect(comparePositions(at(9), at(30, 'runtime-b'))).toBe('unrelated');
  });

  it('separates continuing from skipping', () => {
    expect(comparePositions(at(4), at(5))).toBe('next');
    expect(comparePositions(at(4), at(7))).toBe('ahead-with-gap');
    expect(comparePositions(at(4), at(4))).toBe('not-ahead');
    expect(comparePositions(at(4), at(2))).toBe('not-ahead');
  });

  it('adopts a baseline when joining mid-stream instead of reporting a gap', () => {
    // Cursors are per Session and never reset, so opening an existing Session
    // legitimately starts at a high cursor. Nothing this client applied is
    // missing; what came before is history.
    expect(comparePositions(null, at(1))).toBe('next');
    expect(comparePositions(null, at(6))).toBe('next');
  });
});

describe('SessionStream', () => {
  it('drops a write that is not ahead instead of merging it', () => {
    const stream = sessionStream('local', 'session');
    const applied: number[] = [];
    const offer = (cursor: number) =>
      stream.offer(TEXT, textAt('turn'), at(cursor), () => applied.push(cursor));

    expect(offer(1)).toBe('apply');
    expect(offer(2)).toBe('apply');
    // A redelivery of an event already accounted for.
    expect(offer(2)).toBe('drop');
    expect(offer(1)).toBe('drop');
    expect(stream.appliedPosition()).toEqual(at(2));
  });

  it('reports a gap from the order alone, and clears it when the range arrives', () => {
    const stream = sessionStream('local', 'session');
    stream.offer(TEXT, textAt('turn'), at(1), () => {});
    expect(stream.hasGap()).toBe(false);

    stream.offer(TEXT, textAt('turn'), at(5), () => {});
    expect(stream.hasGap()).toBe(true);

    // A backfill painted 2..5 and reported where it ended.
    stream.commitAppliedPosition(at(5));
    expect(stream.hasGap()).toBe(false);
  });

  it('stops the persisted record from writing an executing Turn', () => {
    const stream = sessionStream('local', 'session');
    stream.offer(STARTED, textAt('turn-1'), at(1), () => {});

    // The Turn is running: its persisted checkpoint is identity only.
    expect(stream.executingTurnIds()).toEqual(['turn-1']);

    stream.offer(COMPLETED, textAt('turn-1'), at(2), () => {});
    expect(stream.executingTurnIds()).toEqual([]);
  });

  it('drops a live event that arrives after its Turn settled', () => {
    const stream = sessionStream('local', 'session');
    stream.offer(STARTED, textAt('turn-1'), at(1), () => {});
    stream.offer(COMPLETED, textAt('turn-1'), at(2), () => {});

    // Ownership transferred to the persisted record; a straggler must not
    // repaint content that record now owns.
    expect(stream.offer(TEXT, textAt('turn-1'), at(3), () => {})).toBe('drop');
  });

  it('holds writes during a read and releases only those ahead of it', () => {
    const stream = sessionStream('local', 'session');
    stream.offer(TEXT, textAt('turn'), at(2), () => {});

    const released: number[] = [];
    const read = stream.beginRead();
    for (const cursor of [3, 6, 7]) {
      expect(
        stream.offer(TEXT, textAt('turn'), at(cursor), () => released.push(cursor)),
      ).toBe('hold');
    }
    expect(released).toEqual([]);

    // The read established everything through cursor 5.
    read.settle(at(5));
    expect(released).toEqual([6, 7]);
    expect(stream.appliedPosition()).toEqual(at(7));
  });

  it('lets an overlapping read inherit held writes instead of double-applying', () => {
    const stream = sessionStream('local', 'session');
    const released: number[] = [];
    const first = stream.beginRead();
    stream.offer(TEXT, textAt('turn'), at(4), () => released.push(4));

    const second = stream.beginRead();
    expect(first.isCurrent()).toBe(false);
    // The superseded read must not drain onto a projection the newer read is
    // about to establish.
    first.settle(at(9));
    expect(released).toEqual([]);

    second.settle(at(3));
    expect(released).toEqual([4]);
  });

  it('supersedes the old ordering wholesale when the Runtime process changes', () => {
    const stream = sessionStream('local', 'session');
    stream.offer(STARTED, textAt('turn-1'), at(1), () => {});
    stream.offer(COMPLETED, textAt('turn-1'), at(2), () => {});

    // A restarted Host mints a new stream. Its low cursors are not "behind",
    // and the old Turn's settled state says nothing about the new process.
    expect(
      stream.offer(TEXT, textAt('turn-1'), at(1, 'runtime-b'), () => {}),
    ).toBe('apply');
    expect(stream.appliedPosition()).toEqual(at(1, 'runtime-b'));
  });

  it('applies an unpositioned write rather than hiding it', () => {
    const stream = sessionStream('local', 'session');
    stream.offer(TEXT, textAt('turn'), at(5), () => {});
    // An older Host, or a Session-scoped event with no cursor: there is no
    // ordering to violate.
    expect(stream.offer('session_title_generated', { sessionId: 'session' }, null, () => {}))
      .toBe('apply');
    expect(stream.appliedPosition()).toEqual(at(5));
  });

  it('keeps identity per surface so two devices cannot share a position', () => {
    const local = sessionStream('local', 'session');
    const peer = sessionStream('device-b', 'session');

    local.offer(TEXT, textAt('turn'), at(7), () => {});
    expect(peer.appliedPosition()).toBeNull();
    expect(sessionStream('local', 'session')).toBe(local);
    expect(sessionStream('device-b', 'session')).toBe(peer);
  });
});
