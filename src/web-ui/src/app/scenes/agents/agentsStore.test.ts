import { beforeEach, describe, expect, it } from 'vitest';
import {
  MOCK_AGENT_TEAMS,
  computeAgentTeamCapabilities,
  useAgentsStore,
} from './agentsStore';

describe('agentsStore team state (recovered by R-WF-13)', () => {
  it('seeds mock agent teams with the first team active', () => {
    const state = useAgentsStore.getState();
    expect(state.agentTeams).toHaveLength(MOCK_AGENT_TEAMS.length);
    expect(state.activeAgentTeamId).toBe(MOCK_AGENT_TEAMS[0].id);
    expect(state.viewMode).toBe('formation');
  });

  it('adds an agent team and selects it', () => {
    const { addAgentTeam } = useAgentsStore.getState();
    addAgentTeam({
      id: 'agent-team-test-1',
      name: 'Test Team',
      icon: 'rocket',
      description: '',
      strategy: 'collaborative',
      shareContext: true,
    });
    const state = useAgentsStore.getState();
    expect(state.agentTeams.some((t) => t.id === 'agent-team-test-1')).toBe(true);
    expect(state.activeAgentTeamId).toBe('agent-team-test-1');
    // cleanup
    useAgentsStore.getState().deleteAgentTeam('agent-team-test-1');
  });

  it('adds/removes members and updates roles', () => {
    const teamId = 'agent-team-coding';
    const { addMember, removeMember, updateMemberRole } = useAgentsStore.getState();
    addMember(teamId, 'agentic', 'leader');
    let team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(team.members.some((m) => m.agentId === 'agentic')).toBe(true);

    updateMemberRole(teamId, 'agentic', 'reviewer');
    team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(team.members.find((m) => m.agentId === 'agentic')?.role).toBe('reviewer');

    removeMember(teamId, 'agentic');
    team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
    expect(team.members.some((m) => m.agentId === 'agentic')).toBe(false);
  });

  it('computes team capability coverage from member agents', () => {
    const team = MOCK_AGENT_TEAMS[0];
    const agents = [
      { id: 'agentic', capabilities: [{ category: 'coding' as const, level: 5 }, { category: 'analysis' as const, level: 4 }] },
      { id: 'CodeReview', capabilities: [{ category: 'coding' as const, level: 4 }, { category: 'testing' as const, level: 3 }] },
      { id: 'Debug', capabilities: [{ category: 'coding' as const, level: 5 }, { category: 'testing' as const, level: 4 }] },
    ];
    const coverage = computeAgentTeamCapabilities(
      team,
      agents as unknown as Parameters<typeof computeAgentTeamCapabilities>[1],
    );
    expect(coverage.coding).toBeGreaterThan(0);
    expect(coverage.analysis).toBeGreaterThan(0);
    expect(coverage.testing).toBeGreaterThan(0);
  });

  describe('R-WF-17 DAG edge editing', () => {
    const teamId = 'agent-team-coding';

    beforeEach(() => {
      // Isolate from the global store mutations made by earlier tests.
      const { deleteAgentTeam, addAgentTeam } = useAgentsStore.getState();
      deleteAgentTeam(teamId);
      const base = MOCK_AGENT_TEAMS.find((t) => t.id === teamId)!;
      addAgentTeam({
        id: teamId,
        name: base.name,
        icon: base.icon,
        description: base.description,
        strategy: base.strategy,
        shareContext: base.shareContext,
      });
      const { addMember, setMemberDisplayStates } = useAgentsStore.getState();
      for (const member of base.members) {
        addMember(teamId, member.agentId, member.role);
      }
      setMemberDisplayStates(teamId, Object.fromEntries(
        base.members.map((m) => [m.agentId, m.displayState ?? 'standby']),
      ));
      // Restore the mock edges.
      const { addTeamEdge } = useAgentsStore.getState();
      for (const [a, b] of base.edges) {
        addTeamEdge(teamId, a, b);
      }
    });

    it('adds a member edge when both endpoints are members and distinct', () => {
      const { addTeamEdge } = useAgentsStore.getState();
      addTeamEdge(teamId, 'agentic', 'GeneralPurpose');
      const team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
      expect(team.edges.some(([a, b]) => a === 'agentic' && b === 'GeneralPurpose')).toBe(true);

      // self-loop rejected
      addTeamEdge(teamId, 'agentic', 'agentic');
      addTeamEdge(teamId, 'missing', 'agentic');
      const after = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
      expect(after.edges.filter(([a, b]) => a === 'agentic' && b === 'agentic')).toHaveLength(0);
    });

    it('does not duplicate an existing edge', () => {
      const { addTeamEdge } = useAgentsStore.getState();
      addTeamEdge(teamId, 'agentic', 'Debug');
      const team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
      expect(team.edges.filter(([a, b]) => a === 'agentic' && b === 'Debug')).toHaveLength(1);
    });

    it('removes an edge', () => {
      const { removeTeamEdge } = useAgentsStore.getState();
      removeTeamEdge(teamId, 'agentic', 'CodeReview');
      const team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
      expect(team.edges.some(([a, b]) => a === 'agentic' && b === 'CodeReview')).toBe(false);
    });

    it('updates and bulk-sets member display states (7 states)', () => {
      const { updateMemberDisplayState, setMemberDisplayStates } = useAgentsStore.getState();
      updateMemberDisplayState(teamId, 'agentic', 'interrupted');
      let team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
      expect(team.members.find((m) => m.agentId === 'agentic')?.displayState).toBe('interrupted');

      setMemberDisplayStates(teamId, { agentic: 'hung', CodeReview: 'pending_attention' });
      team = useAgentsStore.getState().agentTeams.find((t) => t.id === teamId)!;
      expect(team.members.find((m) => m.agentId === 'agentic')?.displayState).toBe('hung');
      expect(team.members.find((m) => m.agentId === 'CodeReview')?.displayState).toBe('pending_attention');
    });

    it('seeds seven-state coverage in the mock teams', () => {
      const states = new Set<string>();
      for (const team of MOCK_AGENT_TEAMS) {
        for (const member of team.members) {
          if (member.displayState) states.add(member.displayState);
        }
      }
      for (const expected of ['standby', 'processing', 'completed', 'hung', 'interrupted', 'pending_attention', 'viewed']) {
        expect(states).toContain(expected);
      }
    });
  });
});
