import { create } from 'zustand';
import type { ToastType } from '../components/shared/Toast';
import {
  checkAppUpdate,
  downloadAndInstallAppUpdate,
  getCurrentAppVersion,
  restartAppForUpdate,
} from '../lib/updateApi';
import type { AppUpdateInfo, AppUpdateProgressEvent } from '../lib/updateApi';
import { isTauriAvailable } from '../lib/settingsApi';
import { useSettingsStore } from './settings';

const AUTO_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;

export type UpdateInstallState = 'idle' | 'checking' | 'available' | 'downloading' | 'restart_required' | 'up_to_date';

interface UpdateState {
  currentVersion: string | null;
  activeInfo: AppUpdateInfo | null;
  dialogOpen: boolean;
  checking: boolean;
  installState: UpdateInstallState;
  progress: AppUpdateProgressEvent | null;
  error: string | null;
  initialized: boolean;
  toastMessage: string | null;
  toastType: ToastType;

  hydrateCurrentVersion: () => Promise<void>;
  autoCheckIfDue: () => Promise<void>;
  checkForUpdates: (manual?: boolean) => Promise<AppUpdateInfo | null>;
  applyProgressEvent: (event: AppUpdateProgressEvent) => void;
  downloadAndInstallAvailableUpdate: () => Promise<void>;
  restartToApplyUpdate: () => Promise<void>;
  openDialog: () => void;
  closeDialog: () => void;
  skipCurrentVersion: () => void;
  clearError: () => void;
  showToast: (message: string, type?: ToastType) => void;
  clearToast: () => void;
}

function shouldAutoCheck(lastCheckedAt: string | null): boolean {
  if (!lastCheckedAt) return true;
  const lastCheckedTime = Date.parse(lastCheckedAt);
  if (Number.isNaN(lastCheckedTime)) return true;
  return Date.now() - lastCheckedTime >= AUTO_CHECK_INTERVAL_MS;
}

export const useUpdateStore = create<UpdateState>()((set, get) => ({
  currentVersion: null,
  activeInfo: null,
  dialogOpen: false,
  checking: false,
  installState: 'idle',
  progress: null,
  error: null,
  initialized: false,
  toastMessage: null,
  toastType: 'info',

  hydrateCurrentVersion: async () => {
    if (!isTauriAvailable()) {
      set({ initialized: true });
      return;
    }
    try {
      const currentVersion = await getCurrentAppVersion();
      set({ currentVersion, initialized: true });
    } catch (error) {
      set({
        initialized: true,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  },

  autoCheckIfDue: async () => {
    const { updatePreferences } = useSettingsStore.getState();
    if (!isTauriAvailable() || !updatePreferences.autoCheckForUpdates) return;
    if (!shouldAutoCheck(updatePreferences.lastUpdateCheckAt)) return;
    await get().checkForUpdates(false);
  },

  checkForUpdates: async (manual = false) => {
    if (!isTauriAvailable()) return null;
    const settings = useSettingsStore.getState();
    const channel = settings.updatePreferences.updateChannel;
    set({ checking: true, error: null, installState: 'checking' });
    try {
      if (!get().currentVersion) {
        await get().hydrateCurrentVersion();
      }
      const info = await checkAppUpdate(channel);
      settings.setLastUpdateCheckAt(new Date().toISOString());
      const ignoredVersion = settings.updatePreferences.ignoredUpdateVersionByChannel[channel];
      const isIgnored = !!info.target_version && ignoredVersion === info.target_version;

      if (!info.available) {
        set({
          checking: false,
          activeInfo: info,
          installState: 'up_to_date',
          dialogOpen: false,
        });
        if (manual) {
          get().showToast('updates.toasts.upToDate', 'info');
        }
        return info;
      }

      set({
        checking: false,
        activeInfo: info,
        installState: 'available',
        dialogOpen: manual || !isIgnored,
      });
      return info;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ checking: false, error: message, installState: 'idle' });
      get().showToast(message, 'error');
      return null;
    }
  },

  applyProgressEvent: (progress) => {
    const installState =
      progress.stage === 'finished'
        ? 'restart_required'
        : progress.stage === 'downloading' || progress.stage === 'verifying'
          ? 'downloading'
          : get().installState;
    set({ progress, installState });
  },

  downloadAndInstallAvailableUpdate: async () => {
    const info = get().activeInfo;
    if (!info?.available || !info.target_version) {
      return;
    }
    set({ installState: 'downloading', error: null, progress: null });
    try {
      const result = await downloadAndInstallAppUpdate(info.channel, info.target_version);
      set({
        installState: result.restart_required ? 'restart_required' : 'idle',
        dialogOpen: result.restart_required,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ error: message, installState: 'available' });
      get().showToast(message, 'error');
    }
  },

  restartToApplyUpdate: async () => {
    try {
      await restartAppForUpdate();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ error: message });
      get().showToast(message, 'error');
    }
  },

  openDialog: () => set({ dialogOpen: true }),
  closeDialog: () => set({ dialogOpen: false }),

  skipCurrentVersion: () => {
    const info = get().activeInfo;
    if (!info?.target_version) return;
    useSettingsStore.getState().setIgnoredUpdateVersion(info.channel, info.target_version);
    set({ dialogOpen: false });
  },

  clearError: () => set({ error: null }),
  showToast: (toastMessage, toastType = 'info') => set({ toastMessage, toastType }),
  clearToast: () => set({ toastMessage: null }),
}));
