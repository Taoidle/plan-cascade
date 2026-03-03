import type { PluginInvocation } from '../../types/plugin';
import { reportNonFatal } from '../../lib/nonFatal';

const PLUGIN_SKILL_LINE_REGEX = /^\s*\/([A-Za-z0-9._-]+):([A-Za-z0-9._-]+)(?:\s+(.*))?\s*$/;

export function parsePluginInvocationArgs(rawArgs: string | undefined): Record<string, string> {
  if (!rawArgs) return {};
  const args: Record<string, string> = {};
  const argRegex = /([A-Za-z0-9_.-]+)=("([^"]*)"|'([^']*)'|([^\s]+))/g;
  let match: RegExpExecArray | null = null;
  while ((match = argRegex.exec(rawArgs)) !== null) {
    const key = match[1];
    const value = match[3] ?? match[4] ?? match[5] ?? '';
    args[key] = value;
  }
  return args;
}

export function extractPluginInvocationsFromPrompt(prompt: string): {
  cleanedPrompt: string;
  pluginInvocations: PluginInvocation[];
} {
  const lines = prompt.split('\n');
  const cleanedLines: string[] = [];
  const pluginInvocations: PluginInvocation[] = [];

  for (const line of lines) {
    const matched = line.match(PLUGIN_SKILL_LINE_REGEX);
    if (!matched) {
      cleanedLines.push(line);
      continue;
    }
    pluginInvocations.push({
      plugin_name: matched[1],
      skill_name: matched[2],
      args: parsePluginInvocationArgs(matched[3]),
      source: 'slash',
    });
  }

  return {
    cleanedPrompt: cleanedLines.join('\n').trim(),
    pluginInvocations,
  };
}

export function ensurePromptContent(prompt: string, invocationCount: number): string {
  if (prompt.trim().length > 0) return prompt;
  if (invocationCount > 0) return 'Apply the selected plugin skill instructions.';
  return prompt;
}

export function formatToolArgs(toolName: string, rawArgs?: string): string {
  if (!rawArgs) return '';
  try {
    const args = JSON.parse(rawArgs) as Record<string, unknown>;
    switch (toolName) {
      case 'Read':
      case 'Write':
      case 'Edit':
      case 'LS':
        return String(args.file_path || args.path || '');
      case 'Bash':
        return String(args.command || '').substring(0, 120);
      case 'Glob':
        return `${args.pattern || ''}${args.path ? ` in ${args.path}` : ''}`;
      case 'Grep':
        return `/${args.pattern || ''}/${args.path ? ` in ${args.path}` : ''}`;
      case 'Cwd':
        return '';
      case 'Task':
        return String(args.prompt || '').substring(0, 120);
      default: {
        const compact = JSON.stringify(args);
        return compact.length > 120 ? compact.substring(0, 120) + '...' : compact;
      }
    }
  } catch (error) {
    reportNonFatal('execution.summarizeToolCallArgs', error);
    return rawArgs.length > 120 ? rawArgs.substring(0, 120) + '...' : rawArgs;
  }
}
