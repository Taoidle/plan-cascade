/**
 * Agent Selector Component
 *
 * Dropdown for selecting AI agent for each story
 * with icons and bulk assignment option.
 */

import { clsx } from 'clsx';
import { usePRDStore, AgentType, PRDStory } from '../../store/prd';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { ChevronDownIcon, PersonIcon, CodeIcon, GearIcon, CheckIcon } from '@radix-ui/react-icons';

interface AgentConfig {
  value: AgentType;
  label: string;
  description: string;
  icon: React.ReactNode;
  color: string;
}

const agents: AgentConfig[] = [
  {
    value: 'claude-code',
    label: 'Claude Code',
    description: 'Anthropic Claude for code tasks',
    icon: <PersonIcon className="w-4 h-4" />,
    color: 'bg-orange-100 dark:bg-orange-900 text-orange-700 dark:text-orange-300',
  },
  {
    value: 'aider',
    label: 'Aider',
    description: 'Aider AI pair programmer',
    icon: <CodeIcon className="w-4 h-4" />,
    color: 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300',
  },
  {
    value: 'codex',
    label: 'Codex',
    description: 'OpenAI Codex agent',
    icon: <GearIcon className="w-4 h-4" />,
    color: 'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300',
  },
];

interface AgentSelectorProps {
  story: PRDStory;
  compact?: boolean;
}

export function AgentSelector({ story, compact = false }: AgentSelectorProps) {
  const { setStoryAgent } = usePRDStore();
  const currentAgent = agents.find((a) => a.value === story.agent) || agents[0];

  if (compact) {
    return (
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            className={clsx(
              'inline-flex items-center gap-1.5 px-2 py-1 rounded-md text-xs',
              currentAgent.color,
              'hover:opacity-80 transition-opacity',
            )}
          >
            {currentAgent.icon}
            <span>{currentAgent.label}</span>
            <ChevronDownIcon className="w-3 h-3" />
          </button>
        </DropdownMenu.Trigger>

        <DropdownMenu.Portal>
          <DropdownMenu.Content
            className={clsx(
              'min-w-[180px] p-1 rounded-lg shadow-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
            )}
            sideOffset={5}
          >
            {agents.map((agent) => (
              <DropdownMenu.Item
                key={agent.value}
                onClick={() => setStoryAgent(story.id, agent.value)}
                className={clsx(
                  'flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer',
                  'text-sm text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-100 dark:hover:bg-gray-700',
                  'focus:outline-none focus:bg-gray-100 dark:focus:bg-gray-700',
                )}
              >
                <span className={clsx('p-1 rounded', agent.color)}>{agent.icon}</span>
                <span className="flex-1">{agent.label}</span>
                {story.agent === agent.value && <CheckIcon className="w-4 h-4 text-primary-600" />}
              </DropdownMenu.Item>
            ))}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    );
  }

  return (
    <div className="space-y-2">
      <label className="block text-xs font-medium text-gray-500 dark:text-gray-400">Agent</label>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            className={clsx(
              'w-full flex items-center gap-2 px-3 py-2 rounded-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'hover:border-gray-300 dark:hover:border-gray-600',
              'text-left transition-colors',
            )}
          >
            <span className={clsx('p-1.5 rounded', currentAgent.color)}>{currentAgent.icon}</span>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-gray-900 dark:text-white">{currentAgent.label}</p>
              <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{currentAgent.description}</p>
            </div>
            <ChevronDownIcon className="w-4 h-4 text-gray-400" />
          </button>
        </DropdownMenu.Trigger>

        <DropdownMenu.Portal>
          <DropdownMenu.Content
            className={clsx(
              'w-[var(--radix-dropdown-menu-trigger-width)] p-1 rounded-lg shadow-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
            )}
            sideOffset={5}
          >
            {agents.map((agent) => (
              <DropdownMenu.Item
                key={agent.value}
                onClick={() => setStoryAgent(story.id, agent.value)}
                className={clsx(
                  'flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer',
                  'text-sm text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-100 dark:hover:bg-gray-700',
                  'focus:outline-none focus:bg-gray-100 dark:focus:bg-gray-700',
                )}
              >
                <span className={clsx('p-1.5 rounded', agent.color)}>{agent.icon}</span>
                <div className="flex-1 min-w-0">
                  <p className="font-medium">{agent.label}</p>
                  <p className="text-xs text-gray-500 dark:text-gray-400">{agent.description}</p>
                </div>
                {story.agent === agent.value && <CheckIcon className="w-4 h-4 text-primary-600" />}
              </DropdownMenu.Item>
            ))}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  );
}

/**
 * Bulk Agent Selector
 *
 * Allows setting the same agent for all stories at once.
 */
export function BulkAgentSelector() {
  const { prd, setBulkAgent } = usePRDStore();

  // Determine if all stories have the same agent
  const allSameAgent = prd.stories.length > 0 && prd.stories.every((s) => s.agent === prd.stories[0].agent);
  const currentAgent = allSameAgent ? agents.find((a) => a.value === prd.stories[0].agent) : null;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">Default Agent</label>
        <span className="text-xs text-gray-500 dark:text-gray-400">Applies to all stories</span>
      </div>

      <div className="grid grid-cols-3 gap-2">
        {agents.map((agent) => (
          <button
            key={agent.value}
            onClick={() => setBulkAgent(agent.value)}
            className={clsx(
              'flex flex-col items-center gap-2 p-3 rounded-lg border-2 transition-all',
              currentAgent?.value === agent.value
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 hover:border-gray-300 dark:hover:border-gray-600',
            )}
          >
            <span className={clsx('p-2 rounded-lg', agent.color)}>{agent.icon}</span>
            <span className="text-sm font-medium text-gray-900 dark:text-white">{agent.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

/**
 * Agent Badge
 *
 * Simple badge showing the agent for a story.
 */
interface AgentBadgeProps {
  agent: AgentType;
}

export function AgentBadge({ agent }: AgentBadgeProps) {
  const config = agents.find((a) => a.value === agent) || agents[0];

  return (
    <span className={clsx('inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs', config.color)}>
      {config.icon}
      <span>{config.label}</span>
    </span>
  );
}

export default AgentSelector;
