/**
 * PersonaIndicatorCard
 *
 * Small inline badge showing which AI persona is currently active.
 * Displays role name and current phase. Non-interactive.
 * Uses gray color scheme to stay subtle.
 */

import { useTranslation } from 'react-i18next';
import type { PersonaIndicatorData } from '../../../types/workflowCard';

const roleIcons: Record<string, string> = {
  TechLead: '\u{1F3AF}',
  SeniorEngineer: '\u{1F50D}',
  BusinessAnalyst: '\u{1F4CB}',
  ProductManager: '\u{1F4CA}',
  SoftwareArchitect: '\u{1F3D7}',
  Developer: '\u{1F4BB}',
  QaEngineer: '\u{2705}',
};

export function PersonaIndicatorCard({ data }: { data: PersonaIndicatorData }) {
  const { t } = useTranslation('simpleMode');
  const icon = roleIcons[data.role] || '\u{1F916}';

  return (
    <div className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700">
      <span className="text-xs">{icon}</span>
      <span className="text-2xs font-medium text-gray-700 dark:text-gray-300">
        {data.displayName}
      </span>
      <span className="text-2xs text-gray-400 dark:text-gray-500">
        {t(`workflow.persona.active`, { name: data.displayName, defaultValue: '{{name}} is working...' })}
      </span>
    </div>
  );
}
