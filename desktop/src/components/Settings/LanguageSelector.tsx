/**
 * LanguageSelector Component
 *
 * Dropdown for selecting the UI language.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useSettingsStore, Language } from '../../store/settings';
import { languageNames, supportedLanguages } from '../../i18n';

export function LanguageSelector() {
  const { t } = useTranslation('settings');
  const { language, setLanguage } = useSettingsStore();

  return (
    <section className="space-y-4">
      <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.language.title')}</h3>
      <select
        value={language}
        onChange={(e) => setLanguage(e.target.value as Language)}
        className={clsx(
          'w-full max-w-xs px-3 py-2 rounded-lg border',
          'border-gray-200 dark:border-gray-700',
          'bg-white dark:bg-gray-800',
          'text-gray-900 dark:text-white',
          'focus:outline-none focus:ring-2 focus:ring-primary-500',
        )}
      >
        {supportedLanguages.map((lang) => (
          <option key={lang} value={lang}>
            {languageNames[lang]}
          </option>
        ))}
      </select>
      <p className="text-sm text-gray-500 dark:text-gray-400">{t('general.language.description')}</p>
    </section>
  );
}

export default LanguageSelector;
