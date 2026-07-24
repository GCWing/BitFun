import { describe, expect, it } from 'vitest';
import { classifyReviewTargetFromFiles } from '../reviewTargetClassifier';
import { buildManagedReviewWorkPackets } from './workPackets';

describe('buildManagedReviewWorkPackets', () => {
  it('creates stable bounded foreground batches for a large target', () => {
    const files = [
      ...Array.from({ length: 45 }, (_, index) => `src/web-ui/src/feature-${index}.ts`),
      ...Array.from({ length: 45 }, (_, index) => `src/crates/services/example-${index}.rs`),
    ];
    const target = classifyReviewTargetFromFiles(files, 'session_files');

    const packets = buildManagedReviewWorkPackets({
      target,
      model: 'default',
      maxFilesPerBatch: 40,
      maxBatches: 8,
      maxParallelInstances: 2,
      timeoutSeconds: 120,
    });

    expect(packets).toHaveLength(4);
    expect(packets.every((packet) => packet.subagentId === 'ReviewWorker')).toBe(true);
    expect(
      packets.every((packet) => packet.roleName === 'Dynamic Review Worker'),
    ).toBe(true);
    expect(packets.every((packet) => packet.assignedScope.files.length <= 40)).toBe(true);
    expect(packets.map((packet) => packet.launchBatch)).toEqual([1, 1, 2, 2]);
    expect(packets.map((packet) => packet.packetId)).toEqual([
      'managed-review:batch-1-of-4',
      'managed-review:batch-2-of-4',
      'managed-review:batch-3-of-4',
      'managed-review:batch-4-of-4',
    ]);
    expect(packets.flatMap((packet) => packet.assignedScope.files).sort())
      .toEqual([...files].sort());
  });

  it('caps planned work instead of creating an unbounded review run', () => {
    const files = Array.from({ length: 500 }, (_, index) => `src/file-${index}.ts`);
    const target = classifyReviewTargetFromFiles(files, 'session_files');

    const packets = buildManagedReviewWorkPackets({
      target,
      model: 'default',
      maxFilesPerBatch: 40,
      maxBatches: 8,
      maxParallelInstances: 2,
      timeoutSeconds: 120,
    });

    expect(packets).toHaveLength(8);
    expect(packets.flatMap((packet) => packet.assignedScope.files)).toHaveLength(320);
  });

  it('honors an ordered evidence priority and a stricter planned-file budget', () => {
    const files = Array.from({ length: 160 }, (_, index) => `src/file-${index}.ts`);
    const target = classifyReviewTargetFromFiles(files, 'session_files');
    const prioritizedFiles = [
      ...files.slice(80, 120),
      ...files.slice(0, 80),
      ...files.slice(120),
    ];

    const packets = buildManagedReviewWorkPackets({
      target,
      model: 'default',
      maxFilesPerBatch: 40,
      maxBatches: 8,
      maxParallelInstances: 2,
      maxPlannedFiles: 128,
      timeoutSeconds: 120,
      eligibleFilePaths: prioritizedFiles,
    });

    const plannedFiles = packets.flatMap((packet) => packet.assignedScope.files);
    expect(plannedFiles).toHaveLength(128);
    expect(plannedFiles).toContain('src/file-80.ts');
    expect(plannedFiles).not.toContain('src/file-159.ts');
  });

  it('spreads the first bounded batches across workspace areas', () => {
    const files = [
      ...Array.from({ length: 120 }, (_, index) => `src/web-ui/src/feature-${index}.ts`),
      'src/crates/services/transport/src/lib.rs',
    ];
    const target = classifyReviewTargetFromFiles(files, 'session_files');

    const packets = buildManagedReviewWorkPackets({
      target,
      model: 'default',
      maxFilesPerBatch: 40,
      maxBatches: 2,
      maxParallelInstances: 2,
      timeoutSeconds: 120,
    });

    expect(packets).toHaveLength(2);
    expect(packets[0].assignedScope.files).toHaveLength(40);
    expect(packets[1].assignedScope.files).toEqual([
      'src/crates/services/transport/src/lib.rs',
    ]);
  });
});
