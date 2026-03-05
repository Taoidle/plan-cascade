type PersonaRole =
  | 'TechLead'
  | 'SeniorEngineer'
  | 'BusinessAnalyst'
  | 'ProductManager'
  | 'SoftwareArchitect'
  | 'Developer'
  | 'QaEngineer';

type TranslationFn = (key: string, options?: { defaultValue?: string }) => string;

interface PersonaTranslationSpec {
  role: PersonaRole;
  key: string;
  defaultValue: string;
}

const PERSONA_TRANSLATIONS: PersonaTranslationSpec[] = [
  { role: 'TechLead', key: 'workflow.persona.TechLead', defaultValue: 'Tech Lead' },
  { role: 'SeniorEngineer', key: 'workflow.persona.SeniorEngineer', defaultValue: 'Senior Engineer' },
  { role: 'BusinessAnalyst', key: 'workflow.persona.BusinessAnalyst', defaultValue: 'Business Analyst' },
  { role: 'ProductManager', key: 'workflow.persona.ProductManager', defaultValue: 'Product Manager' },
  { role: 'SoftwareArchitect', key: 'workflow.persona.SoftwareArchitect', defaultValue: 'Software Architect' },
  { role: 'Developer', key: 'workflow.persona.Developer', defaultValue: 'Developer' },
  { role: 'QaEngineer', key: 'workflow.persona.QaEngineer', defaultValue: 'QA Engineer' },
];

const personaSpecByRole: Record<PersonaRole, PersonaTranslationSpec> = PERSONA_TRANSLATIONS.reduce(
  (acc, item) => {
    acc[item.role] = item;
    return acc;
  },
  {} as Record<PersonaRole, PersonaTranslationSpec>,
);

export function resolvePersonaDisplayName(t: TranslationFn, role: string, fallbackDisplayName?: string | null): string {
  const trimmedFallback = fallbackDisplayName?.trim() ?? '';
  if (trimmedFallback.length > 0) {
    return trimmedFallback;
  }

  const spec = personaSpecByRole[role as PersonaRole];
  if (!spec) {
    return role;
  }
  return t(spec.key, { defaultValue: spec.defaultValue });
}
