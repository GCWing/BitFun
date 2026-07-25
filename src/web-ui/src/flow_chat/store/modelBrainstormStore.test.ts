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

  it('stores selected candidate output as one-time public context', () => {
    useModelBrainstormStore.getState().setPublicContextForSession('session-a', {
      sessionId: 'session-a',
      batchId: 'batch-1',
      candidateId: 'candidate-a',
      modelId: 'model-a',
      modelLabel: 'Model A',
      answer: 'Selected answer',
      createdAt: 200,
    });

    expect(useModelBrainstormStore.getState().publicContexts['session-a']?.answer).toBe('Selected answer');

    const context = useModelBrainstormStore.getState().consumePublicContextForSession('session-a');
    expect(context?.answer).toBe('Selected answer');
    expect(useModelBrainstormStore.getState().publicContexts['session-a']).toBeUndefined();
    expect(useModelBrainstormStore.getState().consumePublicContextForSession('session-a')).toBeUndefined();
  });

  it('finds and consumes public context across the brainstorm lineage', () => {
    useModelBrainstormStore.getState().createBatch({
      ...makeBatch(),
      sourceSessionId: 'source-root',
      candidates: [
        {
          id: 'candidate-a',
          modelId: 'model-a',
          modelLabel: 'Model A',
          sessionId: 'session-a',
          status: 'running',
        },
      ],
    });
    useModelBrainstormStore.getState().setPublicContextForSession('session-a', {
      sessionId: 'session-a',
      batchId: 'batch-1',
      candidateId: 'candidate-a',
      modelId: 'model-a',
      modelLabel: 'Model A',
      answer: 'Selected answer from candidate session',
      createdAt: 300,
    });

    expect(useModelBrainstormStore.getState().getPublicContextForSession('source-root')?.answer)
      .toBe('Selected answer from candidate session');

    const context = useModelBrainstormStore.getState().consumePublicContextForSession('source-root');
    expect(context?.candidateId).toBe('candidate-a');
    expect(useModelBrainstormStore.getState().publicContexts['session-a']).toBeUndefined();
  });

  it('finds the latest reusable candidate session in the same brainstorm lineage', () => {
    useModelBrainstormStore.getState().createBatch({
      ...makeBatch(),
      id: 'batch-1',
      sourceSessionId: 'source-root',
      candidates: [
        {
          id: 'candidate-a1',
          modelId: 'model-a',
          modelLabel: 'Model A',
          sessionId: 'session-a1',
          status: 'running',
        },
        {
          id: 'candidate-b1',
          modelId: 'model-b',
          modelLabel: 'Model B',
          sessionId: 'session-b1',
          status: 'running',
        },
      ],
    });
    useModelBrainstormStore.getState().createBatch({
      ...makeBatch(),
      id: 'batch-2',
      sourceSessionId: 'session-a1',
      createdAt: 300,
      candidates: [
        {
          id: 'candidate-a2',
          modelId: 'model-a',
          modelLabel: 'Model A',
          sessionId: 'session-a1',
          status: 'running',
        },
        {
          id: 'candidate-b2',
          modelId: 'model-b',
          modelLabel: 'Model B',
          sessionId: 'session-b1',
          status: 'running',
        },
      ],
    });
    useModelBrainstormStore.getState().createBatch({
      ...makeBatch(),
      id: 'unrelated',
      sourceSessionId: 'other-root',
      createdAt: 400,
      candidates: [
        {
          id: 'other-candidate',
          modelId: 'model-a',
          modelLabel: 'Model A',
          sessionId: 'other-session-a',
          status: 'running',
        },
      ],
    });

    const reusableSessions = useModelBrainstormStore
      .getState()
      .getReusableCandidateSessionsForSession('session-a1');

    expect(reusableSessions['model-a']).toMatchObject({
      sessionId: 'session-a1',
      batchId: 'batch-2',
      candidateId: 'candidate-a2',
    });
    expect(reusableSessions['model-b']).toMatchObject({
      sessionId: 'session-b1',
      batchId: 'batch-2',
      candidateId: 'candidate-b2',
    });
    expect(reusableSessions['model-a']?.sessionId).not.toBe('other-session-a');
  });
});
