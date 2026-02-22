import React from 'react';
import { ArrowLeft } from 'lucide-react';
import { useTeamStore } from './teamStore';
import AgentsOverviewPage from './components/AgentsOverviewPage';
import ExpertTeamsPage from './components/ExpertTeamsPage';
import TeamTabBar from './components/TeamTabBar';
import AgentGallery from './components/AgentGallery';
import TeamComposer from './components/TeamComposer';
import CapabilityBar from './components/CapabilityBar';
import './TeamView.scss';

const TeamEditorView: React.FC = () => {
  const { openExpertTeamsOverview } = useTeamStore();

  return (
    <div className="tv tv--editor">
      <div className="tv__editor-bar">
        <button className="tv__back-btn" onClick={() => openExpertTeamsOverview()}>
          <ArrowLeft size={14} />
          <span>返回总览</span>
        </button>
      </div>

      <TeamTabBar />

      <div className="tv__body">
        <aside className="tv__gallery">
          <div className="tv__panel-label">Agent 图鉴</div>
          <AgentGallery />
        </aside>

        <main className="tv__composer">
          <TeamComposer />
        </main>
      </div>

      <CapabilityBar />
    </div>
  );
};

const TeamView: React.FC = () => {
  const { page } = useTeamStore();

  if (page === 'editor') return <TeamEditorView />;
  if (page === 'expertTeamsOverview') return <ExpertTeamsPage />;

  return <AgentsOverviewPage />;
};

export default TeamView;
