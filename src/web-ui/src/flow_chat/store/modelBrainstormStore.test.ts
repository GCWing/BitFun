import { describe, expect, it, beforeEach } from 'vitest';
import { useModelBrainstormStore, type ModelBrainstormBatch } from './modelBrainstormStore';

function makeBatch(): ModelBrainstormBatch {
  return {
    id: 'batch-1',
    sourceSessionId: 'source-1',
    question: 'Question with context',
    displayQuestion: 'Question',
    createdAt: 100,
    candidates: [
      {
        id: 'candidate-a',
        modelId: 'model-a',
        modelLabel: 'Model A',
        status: 'pending',
      },
      {
        id: 'candidate-b',
        modelId: 'model-b',
        modelLabel: 'Model B',
        status: 'pending',
      },
    ],
  };
}

describe('modelBrainstormStore', () => {
  beforeEach(() => {
    useModelBrainstormStore.getState().reset();
  });

  it('creates a batch and updates candidate launch state', () => {
    useModelBrainstormStore.getState().createBatch(makeBatch());
    useModelBrainstormStore.getState().updateCandidate('batch-1', 'candidate-a', {
      sessionId: 'session-a',
      status: 'running',
    });

    const batch = useModelBrainstormStore.getState().batches['batch-1'];
    expect(batch.candidates[0]).toMatchObject({
      id: 'candidate-a',
      sessionId: 'session-a',
      status: 'running',
    });
    expect(useModelBrainstormStore.getState().order).toEqual(['batch-1']);
  });

  it('selects only existing candidates', () => {
    useModelBrainstormStore.getState().createBatch(makeBatch());
    useModelBrainstormStore.getState().selectCandidate('batch-1', 'missing');
    expect(useModelBrainstormStore.getState().batches['batch-1'].selectedCandidateId).toBeUndefined();

    useModelBrainstormStore.getState().selectCandidate('batch-1', 'candidate-b');
    expect(useModelBrainstormStore.getState().batches['batch-1'].selectedCandidateId).toBe('candidate-b');
  });

  it('removes batches for a source session', () => {
    useModelBrainstormStore.getState().createBatch(makeBatch());
    useModelBrainstormStore.getState().createBatch({
      ...makeBatch(),
      id: 'batch-2',
      sourceSessionId: 'source-2',
    });

    useModelBrainstormStore.getState().removeBatchesForSession('source-1');

    expect(useModelBrainstormStore.getState().batches['batch-1']).toBeUndefined();
    expect(useModelBrainstormStore.getState().batches['batch-2']).toBeDefined();
    expect(useModelBrainstormStore.getState().order).toEqual(['batch-2']);
  });
});
