/**
 * Prompt Template Types
 *
 * TypeScript types mirroring the Rust PromptTemplate model.
 */

export interface PromptTemplate {
  id: string;
  title: string;
  content: string;
  description: string | null;
  category: string;
  tags: string[];
  variables: string[];
  is_builtin: boolean;
  is_pinned: boolean;
  use_count: number;
  last_used_at: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface PromptCreateRequest {
  title: string;
  content: string;
  description: string | null;
  category: string;
  tags: string[];
  is_pinned: boolean;
}

export interface PromptUpdateRequest {
  title?: string;
  content?: string;
  description?: string | null;
  category?: string;
  tags?: string[];
  is_pinned?: boolean;
}

export interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

export const SYSTEM_PROMPT_CATEGORIES = ['coding', 'writing', 'analysis'] as const;
export const PROMPT_CATEGORIES = ['all', 'uncategorized', ...SYSTEM_PROMPT_CATEGORIES] as const;
export type PromptCategoryId = (typeof PROMPT_CATEGORIES)[number];

/** Map legacy/custom category markers to uncategorized. */
export function normalizePromptCategory(category: string | null | undefined): string {
  const normalized = (category ?? '').trim();
  return normalized.toLowerCase() === 'custom' ? '' : normalized;
}

/** Substitute {{variable}} placeholders in a template string */
export function substituteVariables(template: string, values: Record<string, string>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (match, name) => {
    return values[name] !== undefined ? values[name] : match;
  });
}

/** Extract {{variable}} names from a template string */
export function extractVariables(template: string): string[] {
  const re = /\{\{(\w+)\}\}/g;
  const vars: string[] = [];
  let match;
  while ((match = re.exec(template)) !== null) {
    if (!vars.includes(match[1])) {
      vars.push(match[1]);
    }
  }
  return vars;
}
