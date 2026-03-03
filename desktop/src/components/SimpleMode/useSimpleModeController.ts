import { useMemo } from 'react';
import { useExecutionStore } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';

export interface SimpleModeControllerSnapshot {
  status: string;
  isSubmitting: boolean;
  isRunning: boolean;
  backend: string | null;
  provider: string;
  model: string;
  workspacePath: string | null;
}

export function useSimpleModeController(): SimpleModeControllerSnapshot {
  const status = useExecutionStore((s) => s.status);
  const isSubmitting = useExecutionStore((s) => s.isSubmitting);
  const backend = useSettingsStore((s) => s.backend);
  const provider = useSettingsStore((s) => s.provider);
  const model = useSettingsStore((s) => s.model);
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  return useMemo(
    () => ({
      status,
      isSubmitting,
      isRunning: status === 'running' || status === 'paused',
      backend,
      provider,
      model,
      workspacePath,
    }),
    [status, isSubmitting, backend, provider, model, workspacePath],
  );
}

export default useSimpleModeController;
