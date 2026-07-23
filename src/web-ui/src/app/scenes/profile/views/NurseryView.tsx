import React from 'react';
import { useNurseryStore } from '../nurseryStore';
import NurseryGallery from './NurseryGallery';
import AssistantDefaultsPage from './AssistantDefaultsPage';
import AssistantConfigPage from './AssistantConfigPage';
import './NurseryView.scss';

const NurseryView: React.FC = () => {
  const { page } = useNurseryStore();

  if (page === 'defaults') {
    return <AssistantDefaultsPage />;
  }

  if (page === 'assistant') {
    return <AssistantConfigPage />;
  }

  return <NurseryGallery />;
};

export default NurseryView;
