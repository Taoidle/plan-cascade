import { describe, expect, it } from 'vitest';
import { resolvePersonaDisplayName } from './personaI18n';

describe('personaI18n', () => {
  it('resolves known role with i18n key', () => {
    const t = (key: string, options?: { defaultValue?: string }) => options?.defaultValue || key;
    expect(resolvePersonaDisplayName(t, 'TechLead')).toBe('Tech Lead');
  });

  it('prefers explicit display name when provided', () => {
    const t = (key: string, options?: { defaultValue?: string }) => options?.defaultValue || key;
    expect(resolvePersonaDisplayName(t, 'TechLead', 'Custom Lead')).toBe('Custom Lead');
  });

  it('falls back to role string for unknown role', () => {
    const t = (key: string, options?: { defaultValue?: string }) => options?.defaultValue || key;
    expect(resolvePersonaDisplayName(t, 'UnknownRole')).toBe('UnknownRole');
  });
});
