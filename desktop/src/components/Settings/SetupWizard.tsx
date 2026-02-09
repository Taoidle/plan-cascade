/**
 * SetupWizard Component
 *
 * Multi-step onboarding wizard that guides users through initial configuration.
 * Steps: Welcome -> LLM Provider -> API Key -> Workspace -> Completion
 *
 * Story 007: Onboarding & Setup Wizard
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import * as VisuallyHidden from '@radix-ui/react-visually-hidden';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  CheckCircledIcon,
  Cross2Icon,
  EyeOpenIcon,
  EyeClosedIcon,
  CheckIcon,
  CrossCircledIcon,
  ReloadIcon,
  FileIcon,
} from '@radix-ui/react-icons';
import { useSettingsStore, Backend } from '../../store/settings';

// ============================================================================
// Types
// ============================================================================

interface SetupWizardProps {
  forceShow?: boolean;
  onComplete?: (launchTour: boolean) => void;
}

type WizardStep = 'welcome' | 'provider' | 'apiKey' | 'workspace' | 'complete';

interface StepDef {
  id: WizardStep;
  titleKey: string;
}

// ============================================================================
// Constants
// ============================================================================

const steps: StepDef[] = [
  { id: 'welcome', titleKey: 'steps.welcome' },
  { id: 'provider', titleKey: 'steps.provider' },
  { id: 'apiKey', titleKey: 'steps.apiKey' },
  { id: 'workspace', titleKey: 'steps.workspace' },
  { id: 'complete', titleKey: 'steps.complete' },
];

/** Backends that require an API key */
const BACKENDS_REQUIRING_KEY: Backend[] = ['claude-api', 'openai', 'deepseek', 'glm', 'qwen'];

/** API key format patterns for basic client-side validation */
const API_KEY_PATTERNS: Partial<Record<Backend, RegExp>> = {
  'claude-api': /^sk-ant-[a-zA-Z0-9_-]{20,}$/,
  openai: /^sk-[a-zA-Z0-9_-]{20,}$/,
  deepseek: /^sk-[a-zA-Z0-9_-]{10,}$/,
};

// ============================================================================
// Provider display config
// ============================================================================

interface ProviderOption {
  id: Backend;
  i18nKey: string;
  icon: React.ReactNode;
  needsKey: boolean;
  fallbackName: string;
  fallbackDescription: string;
  fallbackTag?: string;
}

const providerOptions: ProviderOption[] = [
  {
    id: 'claude-code',
    i18nKey: 'claudeCode',
    icon: <ProviderIcon color="var(--color-primary)" letter="C" />,
    needsKey: false,
    fallbackName: 'Claude Code',
    fallbackDescription: 'Use Claude Code subscription directly. No API key required.',
    fallbackTag: 'Recommended',
  },
  {
    id: 'claude-api',
    i18nKey: 'claudeApi',
    icon: <ProviderIcon color="var(--color-accent)" letter="A" />,
    needsKey: true,
    fallbackName: 'Anthropic API',
    fallbackDescription: 'Direct Anthropic Claude API access.',
  },
  {
    id: 'openai',
    i18nKey: 'openai',
    icon: <ProviderIcon color="var(--color-success)" letter="O" />,
    needsKey: true,
    fallbackName: 'OpenAI',
    fallbackDescription: 'Use OpenAI GPT models.',
  },
  {
    id: 'deepseek',
    i18nKey: 'deepseek',
    icon: <ProviderIcon color="var(--color-secondary)" letter="D" />,
    needsKey: true,
    fallbackName: 'DeepSeek',
    fallbackDescription: 'DeepSeek coding models.',
  },
  {
    id: 'glm',
    i18nKey: 'glm',
    icon: <ProviderIcon color="var(--color-info, #0ea5e9)" letter="G" />,
    needsKey: true,
    fallbackName: 'GLM (ZhipuAI)',
    fallbackDescription: 'Use ZhipuAI GLM models.',
  },
  {
    id: 'qwen',
    i18nKey: 'qwen',
    icon: <ProviderIcon color="var(--color-warning)" letter="Q" />,
    needsKey: true,
    fallbackName: 'Qwen (DashScope)',
    fallbackDescription: 'Use Alibaba Qwen models.',
  },
  {
    id: 'ollama',
    i18nKey: 'ollama',
    icon: <ProviderIcon color="var(--color-warning)" letter="L" />,
    needsKey: false,
    fallbackName: 'Ollama (Local)',
    fallbackDescription: 'Run models locally with Ollama.',
    fallbackTag: 'Local',
  },
];

function ProviderIcon({ color, letter }: { color: string; letter: string }) {
  return (
    <div
      className="w-10 h-10 rounded-lg flex items-center justify-center text-white font-bold text-lg"
      style={{ backgroundColor: color }}
    >
      {letter}
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

export function SetupWizard({ forceShow = false, onComplete }: SetupWizardProps) {
  const { t } = useTranslation('wizard');
  const [isOpen, setIsOpen] = useState(false);
  const [currentStep, setCurrentStep] = useState<WizardStep>('welcome');
  const [apiKey, setApiKey] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);
  const [apiKeyStatus, setApiKeyStatus] = useState<'idle' | 'validating' | 'valid' | 'invalid' | 'format_error'>('idle');
  const [isSaving, setIsSaving] = useState(false);
  const [workspacePath, setWorkspacePath] = useState('');
  const [wantsTour, setWantsTour] = useState(true);

  const {
    backend,
    setBackend,
    setProvider,
    setApiKey: storeSetApiKey,
    onboardingCompleted,
    setOnboardingCompleted,
    setWorkspacePath: storeSetWorkspacePath,
  } = useSettingsStore();

  // Determine if wizard should show on mount
  useEffect(() => {
    if (forceShow) {
      setIsOpen(true);
      setCurrentStep('welcome');
      return;
    }

    if (!onboardingCompleted) {
      setIsOpen(true);
    }
  }, [forceShow, onboardingCompleted]);

  // Keyboard shortcuts
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (currentStep === 'complete') {
          handleComplete();
        } else {
          handleNext();
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        handleSkip();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, currentStep, isSaving]);

  const currentStepIndex = steps.findIndex((s) => s.id === currentStep);
  const needsApiKey = BACKENDS_REQUIRING_KEY.includes(backend);

  const handleNext = useCallback(() => {
    const nextIndex = currentStepIndex + 1;
    if (nextIndex < steps.length) {
      setCurrentStep(steps[nextIndex].id);
    }
  }, [currentStepIndex]);

  const handleBack = useCallback(() => {
    const prevIndex = currentStepIndex - 1;
    if (prevIndex >= 0) {
      setCurrentStep(steps[prevIndex].id);
    }
  }, [currentStepIndex]);

  const handleSkip = useCallback(() => {
    setOnboardingCompleted(true);
    setIsOpen(false);
    onComplete?.(false);
  }, [setOnboardingCompleted, onComplete]);

  const handleComplete = useCallback(async () => {
    setIsSaving(true);

    try {
      // Persist API key to OS keyring if set
      if (apiKey && needsApiKey) {
        const provider = getProviderFromBackend(backend);
        const result = await invoke<{ success: boolean; error?: string }>('configure_provider', {
          provider,
          apiKey: apiKey.trim(),
        });

        if (!result.success) {
          throw new Error(result.error || 'Failed to store API key');
        }

        // Keep local copy for backward compatibility with previous flows.
        storeSetApiKey(apiKey);
      }

      // Persist workspace path if set
      if (workspacePath) {
        storeSetWorkspacePath(workspacePath);
      }

      // Save settings to Tauri backend
      await saveSettings();

      setOnboardingCompleted(true);
      setIsOpen(false);
      onComplete?.(wantsTour);
    } catch (error) {
      console.error('Failed to save wizard settings:', error);
    } finally {
      setIsSaving(false);
    }
  }, [apiKey, needsApiKey, backend, workspacePath, wantsTour, storeSetApiKey, storeSetWorkspacePath, setOnboardingCompleted, onComplete]);

  const handleProviderChange = useCallback((newBackend: Backend) => {
    setBackend(newBackend);
    setProvider(getProviderFromBackend(newBackend));
    // Reset API key state when provider changes
    setApiKey('');
    setApiKeyStatus('idle');
    setShowApiKey(false);
  }, [setBackend, setProvider]);

  const validateApiKey = useCallback((key: string) => {
    if (!key) {
      setApiKeyStatus('idle');
      return;
    }

    const pattern = API_KEY_PATTERNS[backend];
    if (pattern) {
      if (pattern.test(key)) {
        // Simulate validation delay for UX
        setApiKeyStatus('validating');
        setTimeout(() => {
          setApiKeyStatus('valid');
        }, 800);
      } else {
        setApiKeyStatus('format_error');
      }
    } else {
      // No pattern available, accept any non-empty key
      setApiKeyStatus('validating');
      setTimeout(() => {
        setApiKeyStatus('valid');
      }, 500);
    }
  }, [backend]);

  const handleApiKeyChange = useCallback((value: string) => {
    setApiKey(value);
    // Debounce validation
    setApiKeyStatus('idle');
    const timer = setTimeout(() => validateApiKey(value), 600);
    return () => clearTimeout(timer);
  }, [validateApiKey]);

  const handleBrowseWorkspace = useCallback(async () => {
    try {
      // Try Tauri file dialog
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('workspace.label'),
      });
      if (selected && typeof selected === 'string') {
        setWorkspacePath(selected);
      }
    } catch {
      // Fallback: prompt user to type path
      const path = window.prompt(t('workspace.label'));
      if (path) {
        setWorkspacePath(path);
      }
    }
  }, [t]);

  return (
    <Dialog.Root open={isOpen} onOpenChange={setIsOpen}>
      <Dialog.Portal>
        <Dialog.Overlay
          className={clsx(
            'fixed inset-0 bg-black/50 backdrop-blur-sm',
            'data-[state=open]:animate-in data-[state=open]:fade-in-0',
            'data-[state=closed]:animate-out data-[state=closed]:fade-out-0'
          )}
        />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-2xl',
            'bg-[var(--surface)] rounded-xl shadow-xl',
            'overflow-hidden',
            'focus:outline-none'
          )}
          onEscapeKeyDown={(e) => e.preventDefault()}
          aria-describedby={undefined}
        >
          <VisuallyHidden.Root asChild>
            <Dialog.Title>Setup Wizard</Dialog.Title>
          </VisuallyHidden.Root>

          {/* Progress Bar */}
          <div className="h-1 bg-[var(--bg-subtle)]">
            <div
              className="h-full bg-[var(--color-primary)] transition-all duration-300 ease-out"
              style={{ width: `${((currentStepIndex + 1) / steps.length) * 100}%` }}
            />
          </div>

          {/* Step Indicators */}
          <div className="flex justify-center gap-3 p-4 border-b border-[var(--border-default)]">
            {steps.map((step, index) => (
              <div
                key={step.id}
                className={clsx(
                  'flex items-center gap-2',
                  index <= currentStepIndex ? 'text-[var(--color-primary)]' : 'text-[var(--text-muted)]'
                )}
              >
                <span
                  className={clsx(
                    'w-7 h-7 rounded-full flex items-center justify-center text-xs font-semibold transition-all duration-200',
                    index < currentStepIndex
                      ? 'bg-[var(--color-primary)] text-[var(--text-inverted)]'
                      : index === currentStepIndex
                      ? 'border-2 border-[var(--color-primary)] text-[var(--color-primary)]'
                      : 'border-2 border-[var(--border-strong)] text-[var(--text-muted)]'
                  )}
                >
                  {index < currentStepIndex ? (
                    <CheckIcon className="w-4 h-4" />
                  ) : (
                    index + 1
                  )}
                </span>
                <span className="text-sm hidden sm:inline font-medium">
                  {t(step.titleKey)}
                </span>
              </div>
            ))}
          </div>

          {/* Content */}
          <div className="p-8 min-h-[340px]">
            {currentStep === 'welcome' && (
              <WelcomeStep />
            )}
            {currentStep === 'provider' && (
              <ProviderStep
                value={backend}
                onChange={handleProviderChange}
              />
            )}
            {currentStep === 'apiKey' && (
              <ApiKeyStep
                backend={backend}
                needsKey={needsApiKey}
                apiKey={apiKey}
                showApiKey={showApiKey}
                status={apiKeyStatus}
                onApiKeyChange={handleApiKeyChange}
                onToggleShow={() => setShowApiKey(!showApiKey)}
              />
            )}
            {currentStep === 'workspace' && (
              <WorkspaceStep
                path={workspacePath}
                onBrowse={handleBrowseWorkspace}
                onPathChange={setWorkspacePath}
              />
            )}
            {currentStep === 'complete' && (
              <CompleteStep
                backend={backend}
                apiKey={apiKey}
                needsKey={needsApiKey}
                workspacePath={workspacePath}
                wantsTour={wantsTour}
                onTourToggle={setWantsTour}
              />
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between p-6 border-t border-[var(--border-default)]">
            <div>
              {currentStep !== 'welcome' && currentStep !== 'complete' && (
                <button
                  onClick={handleSkip}
                  className="text-sm text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
                >
                  {t('actions.skipSetup')}
                </button>
              )}
            </div>

            <div className="flex gap-3">
              {currentStep !== 'welcome' && (
                <button
                  onClick={handleBack}
                  className={clsx(
                    'inline-flex items-center gap-1 px-4 py-2 rounded-lg',
                    'bg-[var(--bg-subtle)] hover:bg-[var(--bg-muted)]',
                    'text-[var(--text-secondary)]',
                    'transition-colors'
                  )}
                >
                  <ChevronLeftIcon className="w-4 h-4" />
                  {t('actions.back')}
                </button>
              )}

              {currentStep === 'complete' ? (
                <button
                  onClick={handleComplete}
                  disabled={isSaving}
                  className={clsx(
                    'inline-flex items-center gap-2 px-6 py-2 rounded-lg',
                    'bg-[var(--color-primary)] text-[var(--text-inverted)]',
                    'hover:bg-[var(--color-primary-hover)]',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                    'transition-colors font-medium'
                  )}
                >
                  {isSaving ? (
                    <>
                      <ReloadIcon className="w-4 h-4 animate-spin" />
                      {t('actions.finishing')}
                    </>
                  ) : (
                    <>
                      <CheckCircledIcon className="w-4 h-4" />
                      {t('actions.finishSetup')}
                    </>
                  )}
                </button>
              ) : (
                <button
                  onClick={handleNext}
                  className={clsx(
                    'inline-flex items-center gap-1 px-5 py-2 rounded-lg',
                    'bg-[var(--color-primary)] text-[var(--text-inverted)]',
                    'hover:bg-[var(--color-primary-hover)]',
                    'transition-colors font-medium'
                  )}
                >
                  {currentStep === 'welcome' ? t('actions.getStarted') : t('actions.next')}
                  <ChevronRightIcon className="w-4 h-4" />
                </button>
              )}
            </div>
          </div>

          {/* Close Button */}
          <button
            onClick={handleSkip}
            className={clsx(
              'absolute top-4 right-4 p-1.5 rounded-lg',
              'hover:bg-[var(--bg-subtle)]',
              'transition-colors'
            )}
            aria-label="Close"
          >
            <Cross2Icon className="w-4 h-4 text-[var(--text-muted)]" />
          </button>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

// ============================================================================
// Step 1: Welcome
// ============================================================================

function WelcomeStep() {
  const { t } = useTranslation('wizard');

  return (
    <div className="text-center flex flex-col items-center justify-center h-full">
      <div
        className={clsx(
          'w-24 h-24 mx-auto mb-8 rounded-2xl',
          'bg-[var(--color-primary-subtle)]',
          'flex items-center justify-center'
        )}
      >
        <svg
          className="w-14 h-14"
          viewBox="0 0 32 32"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect x="4" y="4" width="24" height="6" rx="2" style={{ fill: 'var(--color-primary)' }} />
          <rect x="6" y="12" width="20" height="6" rx="2" style={{ fill: 'var(--color-primary)', opacity: 0.7 }} />
          <rect x="8" y="20" width="16" height="6" rx="2" style={{ fill: 'var(--color-primary)', opacity: 0.45 }} />
        </svg>
      </div>
      <h2 className="text-3xl font-bold text-[var(--text-primary)] mb-3">
        {t('welcome.title')}
      </h2>
      <p className="text-[var(--text-secondary)] max-w-md mx-auto mb-2 leading-relaxed">
        {t('welcome.description')}
      </p>
      <p className="text-sm text-[var(--text-tertiary)]">
        {t('welcome.subtitle')}
      </p>
    </div>
  );
}

// ============================================================================
// Step 2: LLM Provider Selection
// ============================================================================

interface ProviderStepProps {
  value: Backend;
  onChange: (backend: Backend) => void;
}

function ProviderStep({ value, onChange }: ProviderStepProps) {
  const { t } = useTranslation('wizard');

  return (
    <div>
      <h2 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
        {t('provider.title')}
      </h2>
      <p className="text-[var(--text-secondary)] mb-6">
        {t('provider.description')}
      </p>

      <div className="space-y-2">
        {providerOptions.map((option) => {
          const isSelected = value === option.id;
          const name = t(`provider.options.${option.i18nKey}.name`, {
            defaultValue: option.fallbackName,
          });
          const desc = t(`provider.options.${option.i18nKey}.description`, {
            defaultValue: option.fallbackDescription,
          });
          const tag = t(`provider.options.${option.i18nKey}.tag`, {
            defaultValue: option.fallbackTag || '',
          });

          return (
            <button
              key={option.id}
              onClick={() => onChange(option.id)}
              className={clsx(
                'w-full flex items-center gap-4 p-4 rounded-xl border-2 text-left',
                'transition-all duration-200',
                isSelected
                  ? 'border-[var(--color-primary)] bg-[var(--color-primary-subtle)]'
                  : 'border-[var(--border-default)] hover:border-[var(--border-strong)] hover:bg-[var(--bg-subtle)]'
              )}
            >
              {option.icon}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-[var(--text-primary)]">
                    {name}
                  </span>
                  {tag && (
                    <span
                      className={clsx(
                        'text-2xs px-2 py-0.5 rounded-full font-medium',
                        isSelected
                          ? 'bg-[var(--color-primary)] text-[var(--text-inverted)]'
                          : 'bg-[var(--bg-muted)] text-[var(--text-tertiary)]'
                      )}
                    >
                      {tag}
                    </span>
                  )}
                </div>
                <span className="text-sm text-[var(--text-tertiary)] line-clamp-1">
                  {desc}
                </span>
              </div>
              <div
                className={clsx(
                  'w-5 h-5 rounded-full border-2 flex items-center justify-center shrink-0',
                  'transition-all',
                  isSelected
                    ? 'border-[var(--color-primary)] bg-[var(--color-primary)]'
                    : 'border-[var(--border-strong)]'
                )}
              >
                {isSelected && <CheckIcon className="w-3 h-3 text-[var(--text-inverted)]" />}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}

// ============================================================================
// Step 3: API Key Entry
// ============================================================================

interface ApiKeyStepProps {
  backend: Backend;
  needsKey: boolean;
  apiKey: string;
  showApiKey: boolean;
  status: 'idle' | 'validating' | 'valid' | 'invalid' | 'format_error';
  onApiKeyChange: (key: string) => void;
  onToggleShow: () => void;
}

function ApiKeyStep({
  backend,
  needsKey,
  apiKey,
  showApiKey,
  status,
  onApiKeyChange,
  onToggleShow,
}: ApiKeyStepProps) {
  const { t } = useTranslation('wizard');
  const inputRef = useRef<HTMLInputElement>(null);
  const providerName = getProviderDisplayName(backend);

  useEffect(() => {
    if (needsKey && inputRef.current) {
      inputRef.current.focus();
    }
  }, [needsKey]);

  if (!needsKey) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center">
        <div
          className={clsx(
            'w-16 h-16 mx-auto mb-6 rounded-full',
            'bg-[var(--color-success-subtle)]',
            'flex items-center justify-center'
          )}
        >
          <CheckCircledIcon className="w-8 h-8 text-[var(--color-success)]" />
        </div>
        <h2 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
          {t('apiKey.title')}
        </h2>
        <p className="text-[var(--text-secondary)] max-w-md">
          {t('apiKey.noKeyRequired', { provider: providerName })}
        </p>
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
        {t('apiKey.title')}
      </h2>
      <p className="text-[var(--text-secondary)] mb-6">
        {t('apiKey.description', { provider: providerName })}
      </p>

      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-[var(--text-primary)] mb-2">
            {t('apiKey.label')}
          </label>
          <div className="relative">
            <input
              ref={inputRef}
              type={showApiKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => onApiKeyChange(e.target.value)}
              placeholder={t('apiKey.placeholder', { provider: providerName })}
              className={clsx(
                'w-full px-4 py-3 pr-20 rounded-lg border',
                'bg-[var(--surface)] text-[var(--text-primary)]',
                'placeholder:text-[var(--text-muted)]',
                'focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]',
                'font-mono text-sm',
                'transition-colors',
                status === 'valid'
                  ? 'border-[var(--color-success)]'
                  : status === 'format_error' || status === 'invalid'
                  ? 'border-[var(--color-error)]'
                  : 'border-[var(--border-default)]'
              )}
              autoComplete="off"
              spellCheck={false}
            />
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
              {/* Validation indicator */}
              {status === 'validating' && (
                <ReloadIcon className="w-4 h-4 text-[var(--text-muted)] animate-spin" />
              )}
              {status === 'valid' && (
                <CheckCircledIcon className="w-4 h-4 text-[var(--color-success)]" />
              )}
              {(status === 'invalid' || status === 'format_error') && apiKey && (
                <CrossCircledIcon className="w-4 h-4 text-[var(--color-error)]" />
              )}
              {/* Toggle visibility */}
              <button
                type="button"
                onClick={onToggleShow}
                className={clsx(
                  'p-1.5 rounded-md',
                  'hover:bg-[var(--bg-subtle)]',
                  'text-[var(--text-muted)]',
                  'transition-colors'
                )}
                aria-label={showApiKey ? t('apiKey.hide') : t('apiKey.show')}
              >
                {showApiKey ? (
                  <EyeClosedIcon className="w-4 h-4" />
                ) : (
                  <EyeOpenIcon className="w-4 h-4" />
                )}
              </button>
            </div>
          </div>

          {/* Status message */}
          <div className="mt-2 min-h-[1.25rem]">
            {status === 'validating' && (
              <p className="text-sm text-[var(--text-muted)]">
                {t('apiKey.validating')}
              </p>
            )}
            {status === 'valid' && (
              <p className="text-sm text-[var(--color-success)]">
                {t('apiKey.valid')}
              </p>
            )}
            {status === 'invalid' && (
              <p className="text-sm text-[var(--color-error)]">
                {t('apiKey.invalid')}
              </p>
            )}
            {status === 'format_error' && apiKey && (
              <p className="text-sm text-[var(--color-error)]">
                {t('apiKey.formatError')}
              </p>
            )}
            {status === 'idle' && (
              <p className="text-sm text-[var(--text-muted)]">
                {t('apiKey.hint')}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Step 4: Workspace Selection
// ============================================================================

interface WorkspaceStepProps {
  path: string;
  onBrowse: () => void;
  onPathChange: (path: string) => void;
}

function WorkspaceStep({ path, onBrowse, onPathChange }: WorkspaceStepProps) {
  const { t } = useTranslation('wizard');

  return (
    <div>
      <h2 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
        {t('workspace.title')}
      </h2>
      <p className="text-[var(--text-secondary)] mb-6">
        {t('workspace.description')}
      </p>

      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-[var(--text-primary)] mb-2">
            {t('workspace.label')}
          </label>
          <div className="flex gap-2">
            <div className="relative flex-1">
              <FileIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-muted)]" />
              <input
                type="text"
                value={path}
                onChange={(e) => onPathChange(e.target.value)}
                placeholder={t('workspace.placeholder')}
                className={clsx(
                  'w-full pl-10 pr-4 py-3 rounded-lg border',
                  'border-[var(--border-default)]',
                  'bg-[var(--surface)] text-[var(--text-primary)]',
                  'placeholder:text-[var(--text-muted)]',
                  'focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]',
                  'font-mono text-sm',
                  'transition-colors'
                )}
              />
            </div>
            <button
              onClick={onBrowse}
              className={clsx(
                'px-4 py-3 rounded-lg',
                'bg-[var(--bg-subtle)] hover:bg-[var(--bg-muted)]',
                'text-[var(--text-secondary)] font-medium text-sm',
                'border border-[var(--border-default)]',
                'transition-colors'
              )}
            >
              {t('workspace.browse')}
            </button>
          </div>
          <p className="mt-2 text-sm text-[var(--text-muted)]">
            {path ? t('workspace.selected', { path }) : t('workspace.skipHint')}
          </p>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Step 5: Completion Summary
// ============================================================================

interface CompleteStepProps {
  backend: Backend;
  apiKey: string;
  needsKey: boolean;
  workspacePath: string;
  wantsTour: boolean;
  onTourToggle: (wants: boolean) => void;
}

function CompleteStep({
  backend,
  apiKey,
  needsKey,
  workspacePath,
  wantsTour,
  onTourToggle,
}: CompleteStepProps) {
  const { t } = useTranslation('wizard');

  const providerName = getProviderDisplayName(backend);
  const apiKeyDisplay = needsKey
    ? apiKey
      ? t('complete.summary.apiKeyConfigured')
      : t('complete.summary.apiKeyNotSet')
    : t('complete.summary.apiKeyNotRequired');

  const summaryItems = [
    { label: t('complete.summary.provider'), value: providerName },
    { label: t('complete.summary.apiKey'), value: apiKeyDisplay },
    {
      label: t('complete.summary.workspace'),
      value: workspacePath || t('complete.summary.workspaceNotSet'),
    },
  ];

  return (
    <div className="text-center">
      <div
        className={clsx(
          'w-20 h-20 mx-auto mb-6 rounded-full',
          'bg-[var(--color-success-subtle)]',
          'flex items-center justify-center'
        )}
      >
        <CheckCircledIcon className="w-12 h-12 text-[var(--color-success)]" />
      </div>
      <h2 className="text-2xl font-bold text-[var(--text-primary)] mb-2">
        {t('complete.title')}
      </h2>
      <p className="text-[var(--text-secondary)] max-w-md mx-auto mb-6">
        {t('complete.description')}
      </p>

      {/* Configuration Summary */}
      <div
        className={clsx(
          'rounded-xl border border-[var(--border-default)]',
          'bg-[var(--surface-sunken)]',
          'p-4 mb-6 text-left max-w-sm mx-auto'
        )}
      >
        {summaryItems.map((item, i) => (
          <div
            key={item.label}
            className={clsx(
              'flex justify-between items-center py-2',
              i < summaryItems.length - 1 && 'border-b border-[var(--border-subtle)]'
            )}
          >
            <span className="text-sm text-[var(--text-tertiary)]">{item.label}</span>
            <span className="text-sm font-medium text-[var(--text-primary)]">{item.value}</span>
          </div>
        ))}
      </div>

      {/* Tour prompt */}
      <div className="border-t border-[var(--border-default)] pt-4">
        <p className="text-sm text-[var(--text-secondary)] mb-3">
          {t('complete.tourPrompt')}
        </p>
        <div className="flex justify-center gap-3">
          <button
            onClick={() => onTourToggle(true)}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium transition-all',
              wantsTour
                ? 'bg-[var(--color-primary)] text-[var(--text-inverted)]'
                : 'bg-[var(--bg-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-muted)]'
            )}
          >
            {t('complete.launchTour')}
          </button>
          <button
            onClick={() => onTourToggle(false)}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium transition-all',
              !wantsTour
                ? 'bg-[var(--color-primary)] text-[var(--text-inverted)]'
                : 'bg-[var(--bg-subtle)] text-[var(--text-secondary)] hover:bg-[var(--bg-muted)]'
            )}
          >
            {t('complete.skipTour')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ============================================================================
// Helper Functions
// ============================================================================

function getProviderFromBackend(backend: Backend): string {
  switch (backend) {
    case 'claude-code':
    case 'claude-api':
      return 'anthropic';
    case 'openai':
      return 'openai';
    case 'deepseek':
      return 'deepseek';
    case 'glm':
      return 'glm';
    case 'qwen':
      return 'qwen';
    case 'ollama':
      return 'ollama';
    default:
      return 'anthropic';
  }
}

function getProviderDisplayName(backend: Backend): string {
  switch (backend) {
    case 'claude-code':
      return 'Claude Code';
    case 'claude-api':
      return 'Anthropic API';
    case 'openai':
      return 'OpenAI';
    case 'deepseek':
      return 'DeepSeek';
    case 'glm':
      return 'GLM (ZhipuAI)';
    case 'qwen':
      return 'Qwen (DashScope)';
    case 'ollama':
      return 'Ollama';
    default:
      return 'Unknown';
  }
}

async function saveSettings() {
  const settings = useSettingsStore.getState();

  try {
    const { updateSettings, isTauriAvailable } = await import('../../lib/settingsApi');

    if (isTauriAvailable()) {
      await updateSettings({
        theme: settings.theme,
        default_provider: settings.provider,
        default_model: settings.model,
      });
    }
  } catch (error) {
    console.warn('Tauri settings save failed:', error);
  }

  localStorage.setItem(
    'plan-cascade-settings',
    JSON.stringify({
      backend: settings.backend,
      provider: settings.provider,
      defaultMode: settings.defaultMode,
      theme: settings.theme,
      onboardingCompleted: settings.onboardingCompleted,
      workspacePath: settings.workspacePath,
    })
  );
}

export default SetupWizard;
