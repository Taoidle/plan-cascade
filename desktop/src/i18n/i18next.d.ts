/**
 * i18next TypeScript Declarations
 *
 * Disable strict typing to allow cross-namespace access with colon syntax.
 * This enables patterns like t('common:close') when using useTranslation(['agents', 'common']).
 */

import 'i18next';

// Override the default strict typing to allow any string key
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'common';
    // Ensure t() always returns string (never null)
    returnNull: false;
    // Ensure t() always returns string (never empty string)
    returnEmptyString: false;
  }
}
