/**
 * SetupWizard Component
 *
 * First-time setup wizard that guides users through initial configuration.
 */

import { useState, useEffect } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { clsx } from 'clsx';
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  CheckCircledIcon,
  Cross2Icon,
} from '@radix-ui/react-icons';
import { useSettingsStore, Backend } from '../../store/settings';

const WIZARD_COMPLETED_KEY = 'plan-cascade-wizard-completed';

interface SetupWizardProps {
  forceShow?: boolean;
  onComplete?: () => void;
}

type WizardStep = 'welcome' | 'mode' | 'backend' | 'agents' | 'complete';

const steps: { id: WizardStep; title: string }[] = [
  { id: 'welcome', title: 'Welcome' },
  { id: 'mode', title: 'Working Mode' },
  { id: 'backend', title: 'LLM Backend' },
  { id: 'agents', title: 'Agents' },
  { id: 'complete', title: 'Complete' },
];

export function SetupWizard({ forceShow = false, onComplete }: SetupWizardProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [currentStep, setCurrentStep] = useState<WizardStep>('welcome');
  const [apiKey, setApiKey] = useState('');
  const [isSaving, setIsSaving] = useState(false);

  const {
    defaultMode,
    setDefaultMode,
    backend,
    setBackend,
    setProvider,
    agents,
  } = useSettingsStore();

  // Check if wizard should be shown
  useEffect(() => {
    if (forceShow) {
      setIsOpen(true);
      return;
    }

    const completed = localStorage.getItem(WIZARD_COMPLETED_KEY);
    if (!completed) {
      setIsOpen(true);
    }
  }, [forceShow]);

  const currentStepIndex = steps.findIndex((s) => s.id === currentStep);

  const handleNext = () => {
    const nextIndex = currentStepIndex + 1;
    if (nextIndex < steps.length) {
      setCurrentStep(steps[nextIndex].id);
    }
  };

  const handleBack = () => {
    const prevIndex = currentStepIndex - 1;
    if (prevIndex >= 0) {
      setCurrentStep(steps[prevIndex].id);
    }
  };

  const handleSkip = () => {
    markComplete();
    setIsOpen(false);
    onComplete?.();
  };

  const handleComplete = async () => {
    setIsSaving(true);

    try {
      // Save API key if provided and backend requires it
      if (apiKey && needsApiKey(backend)) {
        await fetch('http://127.0.0.1:8765/api/settings/api-key', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            provider: getProviderFromBackend(backend),
            api_key: apiKey,
          }),
        });
      }

      // Save settings
      await saveSettings();

      markComplete();
      setIsOpen(false);
      onComplete?.();
    } catch (error) {
      console.error('Failed to save wizard settings:', error);
    } finally {
      setIsSaving(false);
    }
  };

  const markComplete = () => {
    localStorage.setItem(WIZARD_COMPLETED_KEY, 'true');
  };

  const saveSettings = async () => {
    const settings = useSettingsStore.getState();

    const payload = {
      backend: settings.backend,
      provider: settings.provider,
      default_mode: settings.defaultMode,
      agents: settings.agents.map((a) => ({
        name: a.name,
        enabled: a.enabled,
        command: a.command,
        is_default: a.isDefault,
      })),
    };

    await fetch('http://127.0.0.1:8765/api/settings', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  };

  return (
    <Dialog.Root open={isOpen} onOpenChange={setIsOpen}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-2xl',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'overflow-hidden',
            'focus:outline-none'
          )}
        >
          {/* Progress Bar */}
          <div className="h-1 bg-gray-100 dark:bg-gray-800">
            <div
              className="h-full bg-primary-600 transition-all"
              style={{ width: `${((currentStepIndex + 1) / steps.length) * 100}%` }}
            />
          </div>

          {/* Step Indicators */}
          <div className="flex justify-center gap-2 p-4 border-b border-gray-200 dark:border-gray-800">
            {steps.map((step, index) => (
              <div
                key={step.id}
                className={clsx(
                  'flex items-center gap-2',
                  index <= currentStepIndex ? 'text-primary-600' : 'text-gray-400'
                )}
              >
                <span
                  className={clsx(
                    'w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium',
                    index < currentStepIndex
                      ? 'bg-primary-600 text-white'
                      : index === currentStepIndex
                      ? 'border-2 border-primary-600 text-primary-600'
                      : 'border-2 border-gray-300 dark:border-gray-600 text-gray-400'
                  )}
                >
                  {index < currentStepIndex ? (
                    <CheckCircledIcon className="w-4 h-4" />
                  ) : (
                    index + 1
                  )}
                </span>
                <span className="text-sm hidden sm:inline">{step.title}</span>
              </div>
            ))}
          </div>

          {/* Content */}
          <div className="p-6 min-h-[300px]">
            {currentStep === 'welcome' && (
              <WelcomeStep />
            )}
            {currentStep === 'mode' && (
              <ModeStep
                value={defaultMode}
                onChange={setDefaultMode}
              />
            )}
            {currentStep === 'backend' && (
              <BackendStep
                value={backend}
                onChange={(b) => {
                  setBackend(b);
                  setProvider(getProviderFromBackend(b));
                }}
                apiKey={apiKey}
                onApiKeyChange={setApiKey}
              />
            )}
            {currentStep === 'agents' && (
              <AgentsStep agents={agents} />
            )}
            {currentStep === 'complete' && (
              <CompleteStep />
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between p-6 border-t border-gray-200 dark:border-gray-800">
            <div>
              {currentStep !== 'welcome' && currentStep !== 'complete' && (
                <button
                  onClick={handleSkip}
                  className="text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
                >
                  Skip setup
                </button>
              )}
            </div>

            <div className="flex gap-3">
              {currentStep !== 'welcome' && currentStep !== 'complete' && (
                <button
                  onClick={handleBack}
                  className={clsx(
                    'inline-flex items-center gap-1 px-4 py-2 rounded-lg',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700'
                  )}
                >
                  <ChevronLeftIcon className="w-4 h-4" />
                  Back
                </button>
              )}

              {currentStep === 'complete' ? (
                <button
                  onClick={handleComplete}
                  disabled={isSaving}
                  className={clsx(
                    'inline-flex items-center gap-1 px-6 py-2 rounded-lg',
                    'bg-primary-600 text-white',
                    'hover:bg-primary-700',
                    'disabled:opacity-50'
                  )}
                >
                  {isSaving ? 'Finishing...' : 'Get Started'}
                </button>
              ) : (
                <button
                  onClick={handleNext}
                  className={clsx(
                    'inline-flex items-center gap-1 px-4 py-2 rounded-lg',
                    'bg-primary-600 text-white',
                    'hover:bg-primary-700'
                  )}
                >
                  {currentStep === 'welcome' ? 'Get Started' : 'Next'}
                  <ChevronRightIcon className="w-4 h-4" />
                </button>
              )}
            </div>
          </div>

          {/* Close Button */}
          <button
            onClick={handleSkip}
            className={clsx(
              'absolute top-4 right-4 p-1 rounded-lg',
              'hover:bg-gray-100 dark:hover:bg-gray-800'
            )}
            aria-label="Close"
          >
            <Cross2Icon className="w-4 h-4 text-gray-500" />
          </button>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

// Step Components

function WelcomeStep() {
  return (
    <div className="text-center">
      <div className="w-20 h-20 mx-auto mb-6 bg-primary-100 dark:bg-primary-900/30 rounded-2xl flex items-center justify-center">
        <svg
          className="w-12 h-12"
          viewBox="0 0 32 32"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect x="4" y="4" width="24" height="6" rx="2" className="fill-primary-600" />
          <rect x="6" y="12" width="20" height="6" rx="2" className="fill-primary-500" />
          <rect x="8" y="20" width="16" height="6" rx="2" className="fill-primary-400" />
        </svg>
      </div>
      <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">
        Welcome to Plan Cascade
      </h2>
      <p className="text-gray-500 dark:text-gray-400 max-w-md mx-auto">
        Let's set up your development orchestration environment. This wizard will guide you through
        the essential configuration steps.
      </p>
    </div>
  );
}

interface ModeStepProps {
  value: 'simple' | 'expert';
  onChange: (mode: 'simple' | 'expert') => void;
}

function ModeStep({ value, onChange }: ModeStepProps) {
  return (
    <div>
      <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
        Choose Your Working Mode
      </h2>
      <p className="text-gray-500 dark:text-gray-400 mb-6">
        Select how you want to use Plan Cascade.
      </p>

      <div className="space-y-3">
        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border-2 cursor-pointer',
            'transition-colors',
            value === 'simple'
              ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
              : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
          )}
        >
          <input
            type="radio"
            name="mode"
            value="simple"
            checked={value === 'simple'}
            onChange={() => onChange('simple')}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white">
              Claude Code GUI Mode
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              Simple interface for Claude Code. Just describe your task and let it execute.
              Best for quick tasks and getting started.
            </div>
          </div>
        </label>

        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border-2 cursor-pointer',
            'transition-colors',
            value === 'expert'
              ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
              : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
          )}
        >
          <input
            type="radio"
            name="mode"
            value="expert"
            checked={value === 'expert'}
            onChange={() => onChange('expert')}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white">
              Standalone Orchestration Mode
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              Full PRD generation, multi-agent coordination, and execution strategies.
              Best for complex projects requiring multiple agents.
            </div>
          </div>
        </label>
      </div>
    </div>
  );
}

interface BackendStepProps {
  value: Backend;
  onChange: (backend: Backend) => void;
  apiKey: string;
  onApiKeyChange: (key: string) => void;
}

function BackendStep({ value, onChange, apiKey, onApiKeyChange }: BackendStepProps) {
  const backends: { id: Backend; name: string; description: string; needsKey: boolean }[] = [
    {
      id: 'claude-code',
      name: 'Claude Code (Recommended)',
      description: 'Uses your existing Claude Code subscription. No additional API key needed.',
      needsKey: false,
    },
    {
      id: 'claude-api',
      name: 'Claude API',
      description: 'Direct Anthropic API access. Requires an API key.',
      needsKey: true,
    },
    {
      id: 'openai',
      name: 'OpenAI',
      description: 'Use OpenAI GPT models. Requires an API key.',
      needsKey: true,
    },
    {
      id: 'deepseek',
      name: 'DeepSeek',
      description: 'DeepSeek coding models. Requires an API key.',
      needsKey: true,
    },
    {
      id: 'ollama',
      name: 'Ollama (Local)',
      description: 'Run models locally with Ollama. No API key needed.',
      needsKey: false,
    },
  ];

  const selectedBackend = backends.find((b) => b.id === value);

  return (
    <div>
      <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
        Select LLM Backend
      </h2>
      <p className="text-gray-500 dark:text-gray-400 mb-6">
        Choose the AI provider for code generation.
      </p>

      <div className="space-y-2 mb-6">
        {backends.map((backend) => (
          <label
            key={backend.id}
            className={clsx(
              'flex items-start gap-3 p-3 rounded-lg border cursor-pointer',
              'transition-colors',
              value === backend.id
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
            )}
          >
            <input
              type="radio"
              name="backend"
              value={backend.id}
              checked={value === backend.id}
              onChange={() => onChange(backend.id)}
              className="mt-1 text-primary-600"
            />
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">
                {backend.name}
              </div>
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {backend.description}
              </div>
            </div>
          </label>
        ))}
      </div>

      {selectedBackend?.needsKey && (
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            API Key
          </label>
          <input
            type="password"
            value={apiKey}
            onChange={(e) => onApiKeyChange(e.target.value)}
            placeholder={`Enter your ${selectedBackend.name} API key`}
            className={clsx(
              'w-full px-3 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          />
          <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
            Your API key will be stored securely and never shared.
          </p>
        </div>
      )}
    </div>
  );
}

interface AgentsStepProps {
  agents: { name: string; enabled: boolean; command: string; isDefault: boolean }[];
}

function AgentsStep({ agents }: AgentsStepProps) {
  return (
    <div>
      <h2 className="text-xl font-semibold text-gray-900 dark:text-white mb-2">
        Execution Agents
      </h2>
      <p className="text-gray-500 dark:text-gray-400 mb-6">
        Plan Cascade can use multiple AI agents for different tasks. Here are the pre-configured agents.
      </p>

      <div className="space-y-2">
        {agents.map((agent) => (
          <div
            key={agent.name}
            className={clsx(
              'flex items-center justify-between p-3 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800'
            )}
          >
            <div className="flex items-center gap-3">
              <div
                className={clsx(
                  'w-2 h-2 rounded-full',
                  agent.enabled ? 'bg-green-500' : 'bg-gray-300'
                )}
              />
              <div>
                <div className="font-medium text-gray-900 dark:text-white text-sm">
                  {agent.name}
                </div>
                <div className="text-xs text-gray-500 dark:text-gray-400">
                  Command: {agent.command}
                </div>
              </div>
            </div>
            {agent.isDefault && (
              <span className="text-xs bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400 px-2 py-0.5 rounded">
                Default
              </span>
            )}
          </div>
        ))}
      </div>

      <p className="mt-4 text-sm text-gray-500 dark:text-gray-400">
        You can add, remove, or configure agents later in Settings.
      </p>
    </div>
  );
}

function CompleteStep() {
  return (
    <div className="text-center">
      <div className="w-20 h-20 mx-auto mb-6 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center">
        <CheckCircledIcon className="w-12 h-12 text-green-600 dark:text-green-400" />
      </div>
      <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">
        You're All Set!
      </h2>
      <p className="text-gray-500 dark:text-gray-400 max-w-md mx-auto">
        Plan Cascade is configured and ready to use. You can always adjust these settings later
        by clicking the gear icon in the header.
      </p>
    </div>
  );
}

// Helper functions

function needsApiKey(backend: Backend): boolean {
  return backend === 'claude-api' || backend === 'openai' || backend === 'deepseek';
}

function getProviderFromBackend(backend: Backend): string {
  switch (backend) {
    case 'claude-code':
    case 'claude-api':
      return 'claude';
    case 'openai':
      return 'openai';
    case 'deepseek':
      return 'deepseek';
    case 'ollama':
      return 'ollama';
    default:
      return 'claude';
  }
}

export default SetupWizard;
