import { describe, expect, it, beforeEach } from 'vitest';
import { useModelBrainstormStore, type ModelBrainstormBatch } from './modelBrainstormStore';

function makeBatch(): ModelBrainstormBatch {
  return {
    id: 'batch-1',
    sourceSessionId: 'source-1',
    contextMode: 'independent',
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
    expect(useModelBrainstormStore.getState().batches['batch-1'].selectedCandidateIds).toEqual([]);

    useModelBrainstormStore.getState().selectCandidate('batch-1', 'candidate-b');
    expect(useModelBrainstormStore.getState().batches['batch-1'].selectedCandidateId).toBe('candidate-b');
    expect(useModelBrainstormStore.getState().batches['batch-1'].selectedCandidateIds).toEqual(['candidate-b']);
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

  it('stores completed candidate answers on the candidate ledger', () => {
    useModelBrainstormStore.getState().createBatch(makeBatch());
    useModelBrainstormStore.getState().setCandidateAnswer(
      'batch-1',
      'candidate-a',
      ' Candidate answer ',
      200,
    );

    const candidate = useModelBrainstormStore.getState().batches['batch-1'].candidates[0];
    expect(candidate.answer).toBe('Candidate answer');
    expect(candidate.completedAt).toBe(200);
  });

  it('toggles multiple public selections and keeps selected answers on the ledger', () => {
    useModelBrainstormStore.getState().createBatch(makeBatch());
    useModelBrainstormStore
      .getState()
      .toggleCandidatePublicSelection('batch-1', 'candidate-a', 'Answer A');
    useModelBrainstormStore
      .getState()
      .toggleCandidatePublicSelection('batch-1', 'candidate-b', 'Answer B');

    let batch = useModelBrainstormStore.getState().batches['batch-1'];
    expect(batch.selectedCandidateId).toBe('candidate-a');
    expect(batch.selectedCandidateIds).toEqual(['candidate-a', 'candidate-b']);
    expect(batch.publicSelections?.map(selection => selection.answer)).toEqual(['Answer A', 'Answer B']);
    expect(batch.candidates[0].answer).toBe('Answer A');
    expect(batch.candidates[1].answer).toBe('Answer B');

    useModelBrainstormStore
      .getState()
      .toggleCandidatePublicSelection('batch-1', 'candidate-a', 'Answer A');

    batch = useModelBrainstormStore.getState().batches['batch-1'];
    expect(batch.selectedCandidateId).toBe('candidate-b');
    expect(batch.selectedCandidateIds).toEqual(['candidate-b']);
    expect(batch.publicSelections?.map(selection => selection.candidateId)).toEqual(['candidate-b']);
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
