import React from 'react';
import { NurseryView } from './views';
import './ProfileScene.scss';

const ProfileScene: React.FC = () => (
  <div className="bitfun-profile-scene" data-bf-scene="profile" data-bf-part="root">
    <NurseryView />
  </div>
);

export default ProfileScene;
