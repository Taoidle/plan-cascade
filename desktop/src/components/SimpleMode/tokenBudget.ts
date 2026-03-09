import type { FileAttachmentData, WorkspaceFileReferenceData } from '../../types/attachment';
import { DEFAULT_PROMPT_TOKEN_BUDGET } from '../../lib/promptTokenBudget';
export { DEFAULT_PROMPT_TOKEN_BUDGET };

export interface AttachmentTokenEstimateInput {
  name: string;
  path: string;
  size: number;
  type: FileAttachmentData['type'];
  mimeType?: string;
  content?: string;
  preview?: string;
}

export interface WorkspaceReferenceTokenEstimateInput {
  name: string;
  relativePath: string;
}

export interface PromptTokenEstimateResult {
  estimated_tokens: number;
  prompt_tokens: number;
  attachment_tokens: number;
  attachment_count: number;
  budget_tokens: number;
  remaining_tokens: number;
  exceeds_budget: boolean;
}

const DEFAULT_NON_TEXT_ATTACHMENT_TOKENS = 48;

export function estimateTokensRough(text: string): number {
  if (!text) return 0;
  return Math.ceil(text.length / 4);
}

export function toAttachmentTokenEstimateInput(attachments: FileAttachmentData[]): AttachmentTokenEstimateInput[] {
  return attachments.map((attachment) => ({
    name: attachment.name,
    path: attachment.path,
    size: attachment.size,
    type: attachment.type,
    mimeType: attachment.mimeType,
    content: attachment.inlineContent,
    preview: attachment.inlinePreview,
  }));
}

export function toWorkspaceReferenceTokenEstimateInput(
  references: WorkspaceFileReferenceData[],
): WorkspaceReferenceTokenEstimateInput[] {
  return references.map((reference) => ({
    name: reference.name,
    relativePath: reference.relativePath,
  }));
}

export function estimatePromptTokensFallback(
  prompt: string,
  attachments: FileAttachmentData[],
  references: WorkspaceFileReferenceData[],
  budgetTokens = DEFAULT_PROMPT_TOKEN_BUDGET,
): PromptTokenEstimateResult {
  const prompt_tokens = estimateTokensRough(prompt);
  const attachment_tokens = attachments.reduce((sum, attachment) => {
    if (attachment.type === 'text') {
      if (attachment.inlineContent) {
        return sum + estimateTokensRough(attachment.inlineContent);
      }
      return sum + Math.ceil(attachment.size / 4);
    }
    return sum + DEFAULT_NON_TEXT_ATTACHMENT_TOKENS;
  }, references.length * 12);

  const estimated_tokens = prompt_tokens + attachment_tokens;
  const remaining_tokens = budgetTokens - estimated_tokens;

  return {
    estimated_tokens,
    prompt_tokens,
    attachment_tokens,
    attachment_count: attachments.length,
    budget_tokens: budgetTokens,
    remaining_tokens,
    exceeds_budget: estimated_tokens > budgetTokens,
  };
}

export function formatTokenCount(value: number): string {
  const abs = Math.abs(value);
  if (abs < 1_000) return `${value}`;
  if (abs < 1_000_000) {
    return `${(value / 1_000).toFixed(abs >= 10_000 ? 0 : 1)}k`;
  }
  return `${(value / 1_000_000).toFixed(abs >= 10_000_000 ? 0 : 1)}m`;
}
