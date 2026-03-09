/**
 * First-launch onboarding state.
 * Onboarding Store - using Zustand.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { LocaleId } from '@/infrastructure/i18n/types';
import type { ThemeId } from '@/infrastructure/theme/types';

/**
 * Onboarding step enum.
 */
export type OnboardingStep = 
  | 'language'     // Step 1: Language
  | 'theme'        // Step 2: Theme
  | 'model'        // Step 3: AI model
  | 'completion';  // Step 4: Completion

/**
 * Step order.
 */
export const STEP_ORDER: OnboardingStep[] = [
  'language',
  'theme',
  'model',
  'completion'
];

/**
 * Model configuration data.
 */
export interface OnboardingModelConfig {
  provider: string;
  apiKey: string;
  baseUrl?: string;
  modelName?: string;
  testPassed?: boolean;
  // Fields needed for saving the model config on completion
  format?: 'openai' | 'responses' | 'anthropic' | 'gemini';
  configName?: string;
  customRequestBody?: string;
  skipSslVerify?: boolean;
  customHeaders?: Record<string, string>;
  customHeadersMode?: 'merge' | 'replace';
}

/**
 * Onboarding state.
 */
interface OnboardingState {
  // State
  isFirstLaunch: boolean;
  isOnboardingActive: boolean;
  currentStep: OnboardingStep;
  completedSteps: OnboardingStep[];
  skipped: boolean;
  
  // Configuration data
  selectedLanguage: LocaleId;
  selectedTheme: ThemeId;
  modelConfig: OnboardingModelConfig | null;
  
  // Actions
  startOnboarding: () => void;
  nextStep: () => void;
  prevStep: () => void;
  goToStep: (step: OnboardingStep) => void;
  skipOnboarding: () => void;
  completeOnboarding: () => void;
  resetOnboarding: () => void;
  
  // Configuration updates
  setLanguage: (language: LocaleId) => void;
  setTheme: (theme: ThemeId) => void;
  setModelConfig: (config: OnboardingModelConfig | null) => void;
  markStepCompleted: (step: OnboardingStep) => void;
  
  // For tests: force onboarding without first-launch check
  forceShowOnboarding: () => void;
}

/**
 * Onboarding store.
 */
export const useOnboardingStore = create<OnboardingState>()(
  persist(
    (set, get) => ({
      // Initial state
      isFirstLaunch: true,
      isOnboardingActive: false,
      currentStep: 'language',
      completedSteps: [],
      skipped: false,
      
      selectedLanguage: 'zh-CN',
      selectedTheme: 'bitfun-dark',
      modelConfig: null,
      
      // Start onboarding
      startOnboarding: () => {
        set({
          isOnboardingActive: true,
          currentStep: 'language',
          completedSteps: [],
          skipped: false
        });
      },
      
      // Next step
      nextStep: () => {
        const { currentStep, completedSteps } = get();
        const currentIndex = STEP_ORDER.indexOf(currentStep);
        
        if (currentIndex < STEP_ORDER.length - 1) {
          const nextStep = STEP_ORDER[currentIndex + 1];
          set({
            currentStep: nextStep,
            completedSteps: completedSteps.includes(currentStep) 
              ? completedSteps 
              : [...completedSteps, currentStep]
          });
        }
      },
      
      // Previous step
      prevStep: () => {
        const { currentStep } = get();
        const currentIndex = STEP_ORDER.indexOf(currentStep);
        
        if (currentIndex > 0) {
          set({
            currentStep: STEP_ORDER[currentIndex - 1]
          });
        }
      },
      
      goToStep: (step: OnboardingStep) => {
        const { completedSteps } = get();
        if (completedSteps.includes(step)) {
          set({ currentStep: step });
        }
      },
      
      // Skip onboarding
      skipOnboarding: () => {
        set({
          isOnboardingActive: false,
          isFirstLaunch: false,
          skipped: true
        });
      },
      
      // Complete onboarding
      completeOnboarding: () => {
        const { completedSteps, currentStep } = get();
        set({
          isOnboardingActive: false,
          isFirstLaunch: false,
          completedSteps: [...completedSteps, currentStep],
          skipped: false
        });
      },
      
      // Reset onboarding (for reruns)
      resetOnboarding: () => {
        set({
          isFirstLaunch: true,
          isOnboardingActive: false,
          currentStep: 'language',
          completedSteps: [],
          skipped: false,
          modelConfig: null
        });
      },
      
      // Set language
      setLanguage: (language: LocaleId) => {
        set({ selectedLanguage: language });
      },
      
      // Set theme
      setTheme: (theme: ThemeId) => {
        set({ selectedTheme: theme });
      },
      
      // Set model configuration
      setModelConfig: (config: OnboardingModelConfig | null) => {
        set({ modelConfig: config });
      },
      
      // Mark step completed
      markStepCompleted: (step: OnboardingStep) => {
        const { completedSteps } = get();
        if (!completedSteps.includes(step)) {
          set({ completedSteps: [...completedSteps, step] });
        }
      },
      
      // Force onboarding (for tests)
      forceShowOnboarding: () => {
        set({
          isOnboardingActive: true,
          currentStep: 'language',
          completedSteps: [],
          skipped: false
        });
      }
    }),
    {
      name: 'bitfun-onboarding-state',
      partialize: (state) => ({
        isFirstLaunch: state.isFirstLaunch,
        skipped: state.skipped,
        selectedLanguage: state.selectedLanguage,
        selectedTheme: state.selectedTheme,
        completedSteps: state.completedSteps
      })
    }
  )
);

/**
 * Check whether model config has all required fields filled.
 * - Preset providers: apiKey is required (baseUrl/modelName come from template).
 * - Custom provider: apiKey, baseUrl, and modelName are all required.
 */
export function isModelConfigComplete(config: OnboardingModelConfig | null | undefined): boolean {
  if (!config?.provider) return false;
  if (!config.apiKey?.trim()) return false;
  if (config.provider === 'custom') {
    if (!config.baseUrl?.trim()) return false;
    if (!config.modelName?.trim()) return false;
  }
  return true;
}

// Selectors
export const selectIsOnboardingActive = (state: OnboardingState) => state.isOnboardingActive;
export const selectCurrentStep = (state: OnboardingState) => state.currentStep;
export const selectStepIndex = (state: OnboardingState) => STEP_ORDER.indexOf(state.currentStep);
export const selectTotalSteps = () => STEP_ORDER.length;
