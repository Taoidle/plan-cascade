import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';

export type RightTab = 'collections' | 'documents' | 'retrieval' | 'health';

interface RightPanelTabsProps {
  activeTab: RightTab;
  onChange: (tab: RightTab) => void;
  onBack: () => void;
}

export function RightPanelTabs({ activeTab, onChange, onBack }: RightPanelTabsProps) {
  const { t } = useTranslation('knowledge');

  return (
    <div className="flex items-center border-b border-gray-200 dark:border-gray-700 px-4">
      <button onClick={onBack} className="md:hidden mr-3 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
        </svg>
      </button>

      {(['collections', 'documents', 'retrieval', 'health'] as RightTab[]).map((tab) => (
        <button
          key={tab}
          onClick={() => onChange(tab)}
          className={clsx(
            'px-4 py-3 text-sm font-medium border-b-2 transition-colors',
            activeTab === tab
              ? 'border-primary-500 text-primary-600 dark:text-primary-400'
              : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300',
          )}
        >
          {t(`tabs.${tab}`)}
        </button>
      ))}
    </div>
  );
}
