import React from 'react';
import { NurseryView } from './views';
import './ProfileScene.scss';

const ProfileScene: React.FC = () => (
  <div className="openbitfun-profile-scene" data-openbitfun-scene="profile" data-openbitfun-part="root">
    <NurseryView />
  </div>
);

export default ProfileScene;
