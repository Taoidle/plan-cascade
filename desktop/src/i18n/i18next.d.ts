/**
 * i18next TypeScript Declarations
 *
 * Module augmentation for type-safe translations.
 */

import 'i18next';
import type { resources } from './index';

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'common';
    resources: (typeof resources)['en'];
  }
}
