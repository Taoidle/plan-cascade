/**
 * TabBar Component
 *
 * Tab switching bar for the GitPanel. Shows three tabs (Changes, History, Branches)
 * with active state indicator and optional count badges.
 */

import { clsx } from 'clsx';
import { useGitStore, type GitTabId } from '../../../store/git';

interface TabDef {
  id: GitTabId;
  label: string;
  count?: number;
}

interface TabBarProps {
  tabs: TabDef[];
}

export function TabBar({ tabs }: TabBarProps) {
  const selectedTab = useGitStore((s) => s.selectedTab);
  const setSelectedTab = useGitStore((s) => s.setSelectedTab);

  return (
    <div className="flex items-center border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          onClick={() => setSelectedTab(tab.id)}
          className={clsx(
            'relative px-4 py-2 text-sm font-medium transition-colors',
            'focus:outline-none focus-visible:ring-2 focus-visible:ring-primary-500',
            selectedTab === tab.id
              ? 'text-primary-600 dark:text-primary-400'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
          )}
        >
          <span className="flex items-center gap-1.5">
            {tab.label}
            {typeof tab.count === 'number' && tab.count > 0 && (
              <span
                className={clsx(
                  'text-2xs min-w-[1.125rem] h-[1.125rem] flex items-center justify-center rounded-full px-1',
                  selectedTab === tab.id
                    ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-700 dark:text-primary-300'
                    : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400',
                )}
              >
                {tab.count}
              </span>
            )}
          </span>
          {/* Active indicator bar */}
          {selectedTab === tab.id && (
            <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary-600 dark:bg-primary-400" />
          )}
        </button>
      ))}
    </div>
  );
}

export default TabBar;
