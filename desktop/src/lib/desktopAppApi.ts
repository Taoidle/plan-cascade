import { invoke } from '@tauri-apps/api/core';
import { isTauriAvailable } from './settingsApi';

export async function quitApplication(): Promise<void> {
  if (!isTauriAvailable()) return;
  await invoke('app_quit');
}

export async function showMainWindow(): Promise<void> {
  if (!isTauriAvailable()) return;
  await invoke('app_show_main_window');
}

export async function hideToBackground(): Promise<void> {
  if (!isTauriAvailable()) return;
  await invoke('app_hide_main_window_to_background');
}

export default {
  quitApplication,
  showMainWindow,
  hideToBackground,
};
