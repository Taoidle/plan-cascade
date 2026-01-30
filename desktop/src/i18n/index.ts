/**
 * i18n Configuration
 *
 * Initializes i18next with language detection and translation resources.
 */

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

// English translations
import enCommon from './locales/en/common.json';
import enSimpleMode from './locales/en/simpleMode.json';
import enExpertMode from './locales/en/expertMode.json';
import enClaudeCode from './locales/en/claudeCode.json';
import enSettings from './locales/en/settings.json';
import enWizard from './locales/en/wizard.json';

// Chinese translations
import zhCommon from './locales/zh/common.json';
import zhSimpleMode from './locales/zh/simpleMode.json';
import zhExpertMode from './locales/zh/expertMode.json';
import zhClaudeCode from './locales/zh/claudeCode.json';
import zhSettings from './locales/zh/settings.json';
import zhWizard from './locales/zh/wizard.json';

// Japanese translations
import jaCommon from './locales/ja/common.json';
import jaSimpleMode from './locales/ja/simpleMode.json';
import jaExpertMode from './locales/ja/expertMode.json';
import jaClaudeCode from './locales/ja/claudeCode.json';
import jaSettings from './locales/ja/settings.json';
import jaWizard from './locales/ja/wizard.json';

export const resources = {
  en: {
    common: enCommon,
    simpleMode: enSimpleMode,
    expertMode: enExpertMode,
    claudeCode: enClaudeCode,
    settings: enSettings,
    wizard: enWizard,
  },
  zh: {
    common: zhCommon,
    simpleMode: zhSimpleMode,
    expertMode: zhExpertMode,
    claudeCode: zhClaudeCode,
    settings: zhSettings,
    wizard: zhWizard,
  },
  ja: {
    common: jaCommon,
    simpleMode: jaSimpleMode,
    expertMode: jaExpertMode,
    claudeCode: jaClaudeCode,
    settings: jaSettings,
    wizard: jaWizard,
  },
} as const;

export const supportedLanguages = ['en', 'zh', 'ja'] as const;
export type SupportedLanguage = (typeof supportedLanguages)[number];

export const languageNames: Record<SupportedLanguage, string> = {
  en: 'English',
  zh: '中文',
  ja: '日本語',
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'en',
    defaultNS: 'common',
    ns: ['common', 'simpleMode', 'expertMode', 'claudeCode', 'settings', 'wizard'],
    interpolation: {
      escapeValue: false, // React already handles escaping
    },
    detection: {
      order: ['localStorage', 'navigator'],
      lookupLocalStorage: 'plan-cascade-language',
      caches: ['localStorage'],
    },
  });

export default i18n;
