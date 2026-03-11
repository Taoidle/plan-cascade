import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauriAvailable } from './settingsApi';

export type UpdateChannel = 'stable' | 'beta' | 'alpha';
export type AppUpdateProgressStage = 'started' | 'downloading' | 'verifying' | 'finished' | 'failed';

export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

export interface AppUpdateInfo {
  current_version: string;
  available: boolean;
  target_version: string | null;
  channel: UpdateChannel;
  published_at: number | null;
  notes: string | null;
  manifest_url: string;
  download_url: string | null;
}

export interface AppUpdateInstallResult {
  installed: boolean;
  version: string | null;
  restart_required: boolean;
  channel: UpdateChannel;
  manifest_url: string;
}

export interface AppUpdateProgressEvent {
  channel: UpdateChannel;
  stage: AppUpdateProgressStage;
  version: string | null;
  downloaded_bytes: number;
  total_bytes: number | null;
  message: string | null;
}

export async function getCurrentAppVersion(): Promise<string> {
  const result = await invoke<CommandResponse<string>>('get_version');
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to get current app version');
  }
  return result.data;
}

export async function checkAppUpdate(channel: UpdateChannel): Promise<AppUpdateInfo> {
  const result = await invoke<CommandResponse<AppUpdateInfo>>('check_app_update', { channel });
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to check for updates');
  }
  return result.data;
}

export async function downloadAndInstallAppUpdate(
  channel: UpdateChannel,
  expectedVersion?: string | null,
): Promise<AppUpdateInstallResult> {
  const result = await invoke<CommandResponse<AppUpdateInstallResult>>('download_and_install_app_update', {
    channel,
    expectedVersion: expectedVersion ?? null,
  });
  if (!result.success || !result.data) {
    throw new Error(result.error || 'Failed to download and install update');
  }
  return result.data;
}

export async function restartAppForUpdate(): Promise<boolean> {
  const result = await invoke<CommandResponse<boolean>>('restart_app_for_update');
  if (!result.success || result.data !== true) {
    throw new Error(result.error || 'Failed to restart application');
  }
  return true;
}

export async function listenForAppUpdateProgress(
  handler: (payload: AppUpdateProgressEvent) => void,
): Promise<UnlistenFn | null> {
  if (!isTauriAvailable()) {
    return null;
  }
  return listen<AppUpdateProgressEvent>('app-update-progress', (event) => {
    if (event.payload) {
      handler(event.payload);
    }
  });
}
