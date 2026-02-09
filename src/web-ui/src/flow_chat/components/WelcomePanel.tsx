/**
 * Welcome panel shown in the empty chat state.
 * Displays work-state analysis and onboarding content.
 */

import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { startchatAgentAPI, gitAPI } from '../../infrastructure/api';
import { globalStateAPI } from '../../shared/types';
import type { WorkStateAnalysis, PredictedAction, QuickAction, GitWorkState } from '../../infrastructure/api/service-api/StartchatAgentAPI';
import { useApp } from '../../app/hooks/useApp';
import { StreamText } from '../../component-library';
import { CubeIcon } from '../../app/components/Header/CubeIcon';
import { aiExperienceConfigService } from '@/infrastructure/config/services';
import { createLogger } from '@/shared/utils/logger';
import './WelcomePanel.css';

const log = createLogger('WelcomePanel');

interface WelcomePanelProps {
  /** Callback when a quick action is clicked. */
  onQuickAction?: (command: string) => void;
  /** Custom class name. */
  className?: string;
}

// Session-level AI analysis failure flag to avoid repeated retries.
let aiAnalysisFailedThisSession = false;

export const WelcomePanel: React.FC<WelcomePanelProps> = ({
  onQuickAction,
  className = ''
}) => {
  const { t, i18n } = useTranslation();
  const [analysis, setAnalysis] = useState<WorkStateAnalysis | null>(null);
  const [fallbackGitState, setFallbackGitState] = useState<GitWorkState | null>(null);
  const [aiAnalysisCompleted, setAiAnalysisCompleted] = useState<boolean>(false);
  const [visibleIntentsCount, setVisibleIntentsCount] = useState<number>(0);

  // Streaming sequence state
  const [featureIntroCompleted, setFeatureIntroCompleted] = useState<boolean>(false);
  const [showGitState, setShowGitState] = useState<boolean>(false);
  const [gitStateCompleted, setGitStateCompleted] = useState<boolean>(false);
  const [showSummary, setShowSummary] = useState<boolean>(false);
  const [showIntents, setShowIntents] = useState<boolean>(false);
  
  // Prevent double-trigger to avoid intent list flicker.
  const defaultIntentsStartedRef = useRef<boolean>(false);
  
  const { switchLeftPanelTab } = useApp();
  // AI enhancement failures are handled silently.
  
  // Static greeting content (available immediately).
  const staticGreeting = React.useMemo(() => {
    const hour = new Date().getHours();
    
    let title = t('welcome.greetingAfternoon');
    let subtitle = t('welcome.subtitleAfternoon');
    
    if (hour >= 5 && hour < 12) {
      title = t('welcome.greetingMorning');
      subtitle = t('welcome.subtitleMorning');
    } else if (hour >= 12 && hour < 18) {
      title = t('welcome.greetingAfternoon');
      subtitle = t('welcome.subtitleAfternoon');
    } else if (hour >= 18 && hour < 23) {
      title = t('welcome.greetingEvening');
      subtitle = t('welcome.subtitleEvening');
    } else {
      title = t('welcome.greetingNight');
      subtitle = t('welcome.subtitleNight');
    }
    
    return { title, subtitle };
  }, [t]);

  const featureIntro = t('welcome.featureIntro');

  // Load fallback Git state without AI.
  const loadFallbackGitState = useCallback(async (workspacePath: string) => {
    try {
      const isGitRepo = await gitAPI.isGitRepository(workspacePath);
      if (!isGitRepo) {
        return;
      }

      const gitStatus = await gitAPI.getStatus(workspacePath);

      const gitWorkState: GitWorkState = {
        currentBranch: gitStatus.current_branch,
        unstagedFiles: gitStatus.unstaged.length + gitStatus.untracked.length,
        stagedFiles: gitStatus.staged.length,
        unpushedCommits: gitStatus.ahead,
        aheadBehind: {
          ahead: gitStatus.ahead,
          behind: gitStatus.behind
        },
        modifiedFiles: []
      };

      setFallbackGitState(gitWorkState);
      
      // Show Git state immediately once the intro is done.
      if (featureIntroCompleted) {
        setTimeout(() => setShowGitState(true), 200);
      }
    } catch (err) {
      log.warn('Failed to load fallback git state', err);
    }
  }, [featureIntroCompleted]);

  // Show default intents line by line; each StreamText onComplete triggers the next.
  const startShowingDefaultIntents = useCallback(() => {
    if (defaultIntentsStartedRef.current) return;
    defaultIntentsStartedRef.current = true;

    setTimeout(() => setVisibleIntentsCount(1), 300);
  }, []);

  // After the current intent line finishes, show the next one.
  const onIntentLineComplete = useCallback((index: number) => {
    if (index < 2) setVisibleIntentsCount(index + 2);
  }, []);

  // Load AI enhancements asynchronously without blocking the static content.
  const loadAiEnhancements = useCallback(async () => {
    try {
      if (aiAnalysisFailedThisSession) {
        setAiAnalysisCompleted(true);
        startShowingDefaultIntents();
        return;
      }

      if (!aiExperienceConfigService.isWelcomePanelAIAnalysisEnabled()) {
        setAiAnalysisCompleted(true);
        startShowingDefaultIntents();
        return;
      }

      const workspace = await globalStateAPI.getCurrentWorkspace();
      const currentWorkspacePath = workspace?.rootPath;

      if (!currentWorkspacePath) {
        setAiAnalysisCompleted(true);
        startShowingDefaultIntents();
        return;
      }

      const isGitRepo = await gitAPI.isGitRepository(currentWorkspacePath);
      if (!isGitRepo) {
        setAiAnalysisCompleted(true);
        return;
      }

      const language = i18n.language?.startsWith('zh') ? 'Chinese' : 'English';
      const result = await startchatAgentAPI.quickAnalyzeWorkState(currentWorkspacePath, language);
      
      setAnalysis(result);
      setAiAnalysisCompleted(true);
    } catch (err) {
      log.warn('AI enhancement failed', err);
      aiAnalysisFailedThisSession = true;
      setAiAnalysisCompleted(true);
      startShowingDefaultIntents();
    }
  }, [startShowingDefaultIntents]);

  useEffect(() => {
    const loadInitialState = async () => {
      try {
        const workspace = await globalStateAPI.getCurrentWorkspace();
        if (workspace?.rootPath) {
          await loadFallbackGitState(workspace.rootPath);
          
          loadAiEnhancements();
        }
      } catch (err) {
        log.warn('Failed to load initial state', err);
      }
    };

    loadInitialState();
  }, [loadFallbackGitState, loadAiEnhancements]);

  const handleQuickActionClick = useCallback((command: string) => {
    onQuickAction?.(command);
  }, [onQuickAction]);

  const handleGitStateClick = useCallback(() => {
    switchLeftPanelTab('git');
  }, [switchLeftPanelTab]);


  // Use static greeting content instead of AI-generated copy.
  const greeting = {
    title: staticGreeting.title,
    subtitle: staticGreeting.subtitle
  };

  const currentState = analysis?.currentState;
  
  // Use AI-provided git state or fallback git state.
  const gitState = currentState?.gitState || fallbackGitState;
  
  // Cache AI analysis content so it stays stable across renders.
  const summaryTextRef = useRef<string>('');
  const predictedActionsRef = useRef<PredictedAction[]>([]);
  const quickActionsRef = useRef<QuickAction[]>([]);
  
  if (currentState?.summary && !summaryTextRef.current) {
    summaryTextRef.current = currentState.summary;
  }
  
  if (analysis?.predictedActions && analysis.predictedActions.length > 0 && predictedActionsRef.current.length === 0) {
    predictedActionsRef.current = analysis.predictedActions;
  }
  
  if (analysis?.quickActions && analysis.quickActions.length > 0 && quickActionsRef.current.length === 0) {
    quickActionsRef.current = analysis.quickActions;
  }
  
  const workSummaryText = summaryTextRef.current;
  const predictedActions = predictedActionsRef.current;
  const quickActions = quickActionsRef.current;
  
  const gitStateLastItem = React.useMemo(() => {
    if (!gitState) return null;
    
    if (gitState.unpushedCommits > 0) return 'unpushed';
    if (gitState.stagedFiles > 0) return 'staged';
    if (gitState.unstagedFiles > 0) return 'unstaged';
    if (gitState.aheadBehind && gitState.aheadBehind.behind > 0) return 'behind';
    if (gitState.aheadBehind && gitState.aheadBehind.ahead > 0) return 'ahead';
    return 'branch';
  }, [gitState]);
  
  // Default recommended actions (fallback).
  const defaultActions = [
    {
      description: t('welcome.defaultAction1'),
      command: t('welcome.defaultAction1Command'),
    },
    {
      description: t('welcome.defaultAction2'),
      command: t('welcome.defaultAction2Command'),
    },
    {
      description: t('welcome.defaultAction3'),
      command: t('welcome.defaultAction3Command'),
    },
  ];

  useEffect(() => {
    if (featureIntroCompleted && gitState && !showGitState && !gitStateCompleted) {
      setTimeout(() => setShowGitState(true), 200);
    }
  }, [featureIntroCompleted, gitState, showGitState, gitStateCompleted]);

  useEffect(() => {
    if (featureIntroCompleted && aiAnalysisCompleted && !gitState && !showIntents) {
      setTimeout(() => {
        setShowIntents(true);
        startShowingDefaultIntents();
      }, 200);
    }
  }, [featureIntroCompleted, aiAnalysisCompleted, gitState, showIntents, startShowingDefaultIntents]);

  useEffect(() => {
    if (gitStateCompleted && !showSummary && !showIntents) {
      if (currentState?.summary) {
        setTimeout(() => setShowSummary(true), 200);
      } else if (aiAnalysisCompleted) {
        setTimeout(() => {
          setShowIntents(true);
          startShowingDefaultIntents();
        }, 200);
      }
    }
  }, [gitStateCompleted, showSummary, showIntents, currentState, aiAnalysisCompleted, startShowingDefaultIntents]);

  return (
    <div className={`welcome-panel ${className}`}>
      <div className="welcome-panel__cube-background">
        <CubeIcon size={200} className="welcome-panel__cube-icon" />
      </div>

      <div className="welcome-panel__greeting">
        <h1 className="welcome-panel__greeting-title">{greeting.title}</h1>
        <h1 className="welcome-panel__greeting-title">{t('welcome.aiPartner')}</h1>
        <p className="welcome-panel__greeting-description">{greeting.subtitle}</p>
      </div>

      <div className="welcome-panel__divider"></div>

      <div className="welcome-panel__feature-intro">
        <p className="welcome-panel__feature-text">
          <StreamText 
            key="feature-intro"
            text={featureIntro}
            effect="typewriter"
            speed={30}
            showCursor={!featureIntroCompleted}
            className="welcome-panel__stream-text"
            autoStart={true}
            onComplete={() => {
              setFeatureIntroCompleted(true);
              // Sequencing continues via useEffect.
            }}
          />
        </p>
      </div>


      {showGitState && gitState && (
        <div className="welcome-panel__section welcome-panel__section--ai-enhanced">
          <h2 className="welcome-panel__section-title">
            {t('welcome.lastTimeYouWere')}
          </h2>
          <div className="welcome-panel__current-state">
            <div className="welcome-panel__git-state" onClick={handleGitStateClick}>
              <div className="welcome-panel__git-info welcome-panel__git-info--colored">
                <span className="welcome-panel__git-item">
                  <StreamText 
                    text={t('welcome.branch')}
                    effect="typewriter"
                    speed={20}
                    showCursor={false}
                    className="welcome-panel__stream-text"
                    colorTheme={undefined as any}
                  />
                  <StreamText 
                    text={gitState.currentBranch}
                    effect="typewriter"
                    speed={20}
                    showCursor={gitStateLastItem === 'branch' && !gitStateCompleted}
                    className="welcome-panel__stream-text welcome-panel__git-branch-text"
                    colorTheme={undefined as any}
                    onComplete={() => {
                      if (gitStateLastItem === 'branch') {
                        setGitStateCompleted(true);
                      }
                    }}
                  />
                </span>
                {gitState.aheadBehind && gitState.aheadBehind.ahead > 0 && (
                  <>
                    <span className="welcome-panel__git-separator"> · </span>
                    <span className="welcome-panel__git-item">
                      <StreamText 
                        text={t('welcome.aheadCommits', { count: gitState.aheadBehind.ahead })}
                        effect="typewriter"
                        speed={20}
                        showCursor={gitStateLastItem === 'ahead' && !gitStateCompleted}
                        className="welcome-panel__stream-text welcome-panel__git-ahead-text"
                        colorTheme={undefined as any}
                        onComplete={() => {
                          if (gitStateLastItem === 'ahead') {
                            setGitStateCompleted(true);
                          }
                        }}
                      />
                    </span>
                  </>
                )}
                {gitState.aheadBehind && gitState.aheadBehind.behind > 0 && (
                  <>
                    <span className="welcome-panel__git-separator"> · </span>
                    <span className="welcome-panel__git-item">
                      <StreamText 
                        text={t('welcome.behindCommits', { count: gitState.aheadBehind.behind })}
                        effect="typewriter"
                        speed={20}
                        showCursor={gitStateLastItem === 'behind' && !gitStateCompleted}
                        className="welcome-panel__stream-text welcome-panel__git-behind-text"
                        colorTheme={undefined as any}
                        onComplete={() => {
                          if (gitStateLastItem === 'behind') {
                            setGitStateCompleted(true);
                          }
                        }}
                      />
                    </span>
                  </>
                )}
                {(gitState.unstagedFiles > 0 || gitState.stagedFiles > 0) && (
                  <>
                    <span className="welcome-panel__git-separator"> · </span>
                    <span className="welcome-panel__git-item">
                      <StreamText 
                        text={t('welcome.unstagedFiles', { count: gitState.unstagedFiles + gitState.stagedFiles })}
                        effect="typewriter"
                        speed={20}
                        showCursor={gitStateLastItem === 'unstaged' && !gitStateCompleted}
                        className="welcome-panel__stream-text welcome-panel__git-unstaged-text"
                        colorTheme={undefined as any}
                        onComplete={() => {
                          if (gitStateLastItem === 'unstaged') {
                            setGitStateCompleted(true);
                          }
                        }}
                      />
                    </span>
                  </>
                )}
                {gitState.stagedFiles > 0 && (
                  <>
                    <span className="welcome-panel__git-separator"> · </span>
                    <span className="welcome-panel__git-item">
                      <StreamText 
                        text={t('welcome.stagedFiles', { count: gitState.stagedFiles })}
                        effect="typewriter"
                        speed={20}
                        showCursor={gitStateLastItem === 'staged' && !gitStateCompleted}
                        className="welcome-panel__stream-text welcome-panel__git-staged-text"
                        colorTheme={undefined as any}
                        onComplete={() => {
                          if (gitStateLastItem === 'staged') {
                            setGitStateCompleted(true);
                          }
                        }}
                      />
                    </span>
                  </>
                )}
                {gitState.unpushedCommits > 0 && (
                  <>
                    <span className="welcome-panel__git-separator"> · </span>
                    <span className="welcome-panel__git-item">
                      <StreamText 
                        text={t('welcome.unpushedCommits', { count: gitState.unpushedCommits })}
                        effect="typewriter"
                        speed={20}
                        showCursor={gitStateLastItem === 'unpushed' && !gitStateCompleted}
                        className="welcome-panel__stream-text welcome-panel__git-unpushed-text"
                        colorTheme={undefined as any}
                        onComplete={() => {
                          if (gitStateLastItem === 'unpushed') {
                            setGitStateCompleted(true);
                          }
                        }}
                      />
                    </span>
                  </>
                )}
              </div>
            </div>

            {gitStateCompleted && currentState?.timeInfo?.lastCommitTimeDesc && (
              <div className="welcome-panel__time-info">
                <span className="welcome-panel__time-icon">{t('welcome.timeIcon')}</span>
                <span className="welcome-panel__time-text">
                  {t('welcome.timeSinceLastCommit')}{currentState.timeInfo.lastCommitTimeDesc}
                </span>
              </div>
            )}
          </div>
        </div>
      )}

      {gitStateCompleted && !showSummary && !showIntents && (
        <div className="welcome-panel__ai-analyzing">
          <span className="welcome-panel__ai-analyzing-cursor"></span>
        </div>
      )}

      {showSummary && workSummaryText && (
        <div className="welcome-panel__work-summary" key="work-summary-container">
          <p className="welcome-panel__work-summary-text">
            <StreamText 
              key="work-summary-stream"
              text={workSummaryText}
              effect="typewriter"
              speed={30}
              showCursor={false}
              className="welcome-panel__stream-text"
              autoStart={true}
              onComplete={() => {
                // After summary completes, start showing the intent list.
                setTimeout(() => {
                  setShowIntents(true);
                  setVisibleIntentsCount(1);
                }, 200);
              }}
            />
          </p>
        </div>
      )}

      {showIntents && predictedActions.length > 0 ? (
        <div className="welcome-panel__section">
          <h2 className="welcome-panel__section-title">
            {t('welcome.youMightWant')}
          </h2>
          
          <div className="welcome-panel__intents">
            {predictedActions.slice(0, 3).map((action: PredictedAction, index: number) => {
              if (index >= visibleIntentsCount) return null;

              const isCurrentLine = index === visibleIntentsCount - 1;
              const startIdx = index * 2;
              const intentActions = quickActions.slice(startIdx, startIdx + 2);

              return (
                <div key={`intent-${index}`} className="welcome-panel__intent-card" style={{ animation: 'fadeInUp 0.4s ease-out' }}>
                  <div className="welcome-panel__intent-header">
                    <div className="welcome-panel__intent-number">{index + 1}</div>
                    <div className="welcome-panel__intent-title">
                      {isCurrentLine ? (
                        <StreamText
                          key={`intent-desc-${index}`}
                          text={action.description}
                          effect="typewriter"
                          speed={30}
                          showCursor={true}
                          className="welcome-panel__stream-text"
                          autoStart={true}
                          onComplete={() => onIntentLineComplete(index)}
                        />
                      ) : (
                        action.description
                      )}
                    </div>
                  </div>
                  
                  {intentActions.length > 0 && (
                    <div className="welcome-panel__intent-actions">
                      {intentActions.map((quickAction: QuickAction, actionIdx: number) => (
                        <button
                          key={actionIdx}
                          className="welcome-panel__intent-action-btn"
                          onClick={() => handleQuickActionClick(quickAction.command)}
                          title={quickAction.command}
                        >
                          <svg className="welcome-panel__intent-action-icon" width="14" height="14" viewBox="0 0 14 14" fill="none">
                            <path d="M5 7h6M8 4l3 3-3 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                          </svg>
                          <span className="welcome-panel__intent-action-text">{quickAction.title}</span>
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ) : showIntents && aiAnalysisCompleted && gitState ? (
        <div className="welcome-panel__section">
          <h2 className="welcome-panel__section-title">
            {t('welcome.youMightWant')}
          </h2>
          
          <div className="welcome-panel__intents">
            {defaultActions.map((action, index) => {
              if (index >= visibleIntentsCount) return null;

              const isCurrentLine = index === visibleIntentsCount - 1;
              return (
                <div 
                  key={`default-intent-${index}`}
                  className="welcome-panel__intent-card welcome-panel__intent-card--clickable" 
                  onClick={() => handleQuickActionClick(action.command)}
                  style={{ animation: 'fadeInUp 0.4s ease-out' }}
                >
                  <div className="welcome-panel__intent-header">
                    <div className="welcome-panel__intent-number">{index + 1}</div>
                    <div className="welcome-panel__intent-title">
                      {isCurrentLine ? (
                        <StreamText
                          key={`default-intent-desc-${index}`}
                          text={action.description}
                          effect="typewriter"
                          speed={30}
                          showCursor={true}
                          className="welcome-panel__stream-text"
                          autoStart={true}
                          onComplete={() => onIntentLineComplete(index)}
                        />
                      ) : (
                        action.description
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ) : null}

    </div>
  );
};

export default WelcomePanel;

