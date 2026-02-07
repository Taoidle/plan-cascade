/**
 * Onboarding Store
 *
 * Manages transient UI state for the setup wizard and feature tour.
 * This store is NOT persisted - it only manages open/close triggers.
 *
 * Story 007: Onboarding & Setup Wizard
 */

import { create } from 'zustand';

interface OnboardingState {
  /** Whether to force-show the setup wizard (from Settings re-trigger) */
  forceShowWizard: boolean;
  /** Whether the feature tour is currently active */
  tourActive: boolean;

  /** Trigger the setup wizard to re-open */
  triggerWizard: () => void;
  /** Clear the wizard trigger (after it opens) */
  clearWizardTrigger: () => void;

  /** Start the feature tour */
  startTour: () => void;
  /** End the feature tour */
  endTour: () => void;
}

export const useOnboardingStore = create<OnboardingState>()((set) => ({
  forceShowWizard: false,
  tourActive: false,

  triggerWizard: () => set({ forceShowWizard: true }),
  clearWizardTrigger: () => set({ forceShowWizard: false }),

  startTour: () => set({ tourActive: true }),
  endTour: () => set({ tourActive: false }),
}));

export default useOnboardingStore;
