import React from 'react';
import { PersonaView } from './views';
import './ProfileScene.scss';

interface ProfileSceneProps {
  workspacePath?: string;
}

const ProfileScene: React.FC<ProfileSceneProps> = ({ workspacePath }) => {
  const normalizedWorkspacePath = workspacePath ?? '';

  return (
    <div className="bitfun-profile-scene">
      <PersonaView workspacePath={normalizedWorkspacePath} />
    </div>
  );
};

export default ProfileScene;

