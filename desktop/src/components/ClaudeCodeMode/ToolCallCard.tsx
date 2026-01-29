/**
 * ToolCallCard Component
 *
 * Visualizes tool calls with collapsible sections showing parameters and results.
 * Each tool type has distinct visual styling.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import {
  FileTextIcon,
  Pencil1Icon,
  TerminalIcon,
  MagnifyingGlassIcon,
  FileIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  ReloadIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  CopyIcon,
  GlobeIcon,
} from '@radix-ui/react-icons';
import { ToolCall, ToolType } from '../../store/claudeCode';

// ============================================================================
// ToolCallCard Component
// ============================================================================

interface ToolCallCardProps {
  toolCall: ToolCall;
  compact?: boolean;
}

export function ToolCallCard({ toolCall, compact = false }: ToolCallCardProps) {
  const [isExpanded, setIsExpanded] = useState(!compact);

  const statusConfig = getStatusConfig(toolCall.status);
  const toolConfig = getToolConfig(toolCall.name);

  return (
    <div
      className={clsx(
        'rounded-lg border overflow-hidden transition-all',
        statusConfig.borderColor,
        statusConfig.bgColor
      )}
    >
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={clsx(
          'w-full flex items-center gap-3 px-3 py-2',
          'hover:bg-black/5 dark:hover:bg-white/5',
          'transition-colors text-left'
        )}
      >
        {/* Expand/Collapse icon */}
        <span className="text-gray-400">
          {isExpanded ? (
            <ChevronDownIcon className="w-4 h-4" />
          ) : (
            <ChevronRightIcon className="w-4 h-4" />
          )}
        </span>

        {/* Tool icon */}
        <span className={clsx('p-1.5 rounded', toolConfig.iconBg)}>
          <toolConfig.Icon className={clsx('w-4 h-4', toolConfig.iconColor)} />
        </span>

        {/* Tool name and summary */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm text-gray-900 dark:text-white">
              {toolCall.name}
            </span>
            {toolCall.parameters.file_path && (
              <span className="text-xs text-gray-500 dark:text-gray-400 truncate">
                {truncatePath(toolCall.parameters.file_path)}
              </span>
            )}
            {toolCall.parameters.command && (
              <span className="text-xs text-gray-500 dark:text-gray-400 truncate font-mono">
                {truncateCommand(toolCall.parameters.command)}
              </span>
            )}
            {toolCall.parameters.pattern && (
              <span className="text-xs text-gray-500 dark:text-gray-400 truncate font-mono">
                {toolCall.parameters.pattern}
              </span>
            )}
          </div>
        </div>

        {/* Status indicator */}
        <span className={clsx('flex items-center gap-1', statusConfig.textColor)}>
          <statusConfig.Icon
            className={clsx('w-4 h-4', toolCall.status === 'executing' && 'animate-spin')}
          />
          <span className="text-xs">{statusConfig.label}</span>
        </span>
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <ToolCallContent toolCall={toolCall} />
        </div>
      )}
    </div>
  );
}

// ============================================================================
// ToolCallContent Component
// ============================================================================

interface ToolCallContentProps {
  toolCall: ToolCall;
}

function ToolCallContent({ toolCall }: ToolCallContentProps) {
  switch (toolCall.name) {
    case 'Read':
      return <ReadToolContent toolCall={toolCall} />;
    case 'Write':
      return <WriteToolContent toolCall={toolCall} />;
    case 'Edit':
      return <EditToolContent toolCall={toolCall} />;
    case 'Bash':
      return <BashToolContent toolCall={toolCall} />;
    case 'Glob':
      return <GlobToolContent toolCall={toolCall} />;
    case 'Grep':
      return <GrepToolContent toolCall={toolCall} />;
    case 'WebFetch':
    case 'WebSearch':
      return <WebToolContent toolCall={toolCall} />;
    default:
      return <GenericToolContent toolCall={toolCall} />;
  }
}

// ============================================================================
// Read Tool Content
// ============================================================================

function ReadToolContent({ toolCall }: ToolCallContentProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    if (toolCall.result?.content) {
      navigator.clipboard.writeText(toolCall.result.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      <div className="space-y-1">
        <Label>File Path</Label>
        <CodeBlock>{toolCall.parameters.file_path || 'N/A'}</CodeBlock>
        {(toolCall.parameters.offset !== undefined || toolCall.parameters.limit !== undefined) && (
          <div className="flex gap-4 text-xs text-gray-500">
            {toolCall.parameters.offset !== undefined && (
              <span>Offset: {toolCall.parameters.offset}</span>
            )}
            {toolCall.parameters.limit !== undefined && (
              <span>Limit: {toolCall.parameters.limit}</span>
            )}
          </div>
        )}
      </div>

      {/* Result */}
      {toolCall.result && (
        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <Label>Content</Label>
            <button
              onClick={handleCopy}
              className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            >
              <CopyIcon className="w-3 h-3" />
              {copied ? 'Copied!' : 'Copy'}
            </button>
          </div>
          <pre className="bg-gray-900 text-gray-100 p-3 rounded-lg overflow-x-auto text-xs max-h-64 overflow-y-auto">
            <code>{toolCall.result.content || toolCall.result.output || 'No content'}</code>
          </pre>
        </div>
      )}

      {/* Error */}
      {toolCall.result?.error && (
        <ErrorDisplay error={toolCall.result.error} />
      )}
    </div>
  );
}

// ============================================================================
// Write Tool Content
// ============================================================================

function WriteToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      <div className="space-y-1">
        <Label>File Path</Label>
        <CodeBlock>{toolCall.parameters.file_path || 'N/A'}</CodeBlock>
      </div>

      <div className="space-y-1">
        <Label>Content</Label>
        <pre className="bg-gray-900 text-gray-100 p-3 rounded-lg overflow-x-auto text-xs max-h-64 overflow-y-auto">
          <code>{toolCall.parameters.content || 'No content'}</code>
        </pre>
      </div>

      {/* Result */}
      {toolCall.result && (
        <div className={clsx(
          'flex items-center gap-2 p-2 rounded text-sm',
          toolCall.result.success
            ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400'
            : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400'
        )}>
          {toolCall.result.success ? (
            <>
              <CheckCircledIcon className="w-4 h-4" />
              <span>File written successfully</span>
            </>
          ) : (
            <>
              <CrossCircledIcon className="w-4 h-4" />
              <span>{toolCall.result.error || 'Write failed'}</span>
            </>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Edit Tool Content
// ============================================================================

function EditToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* File path */}
      <div className="space-y-1">
        <Label>File Path</Label>
        <CodeBlock>{toolCall.parameters.file_path || 'N/A'}</CodeBlock>
      </div>

      {/* Diff view */}
      <div className="grid grid-cols-2 gap-2">
        <div className="space-y-1">
          <Label>Old String</Label>
          <pre className="bg-red-50 dark:bg-red-900/20 text-red-800 dark:text-red-200 p-2 rounded text-xs overflow-x-auto max-h-32 overflow-y-auto border border-red-200 dark:border-red-800">
            <code>{toolCall.parameters.old_string || 'N/A'}</code>
          </pre>
        </div>
        <div className="space-y-1">
          <Label>New String</Label>
          <pre className="bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-200 p-2 rounded text-xs overflow-x-auto max-h-32 overflow-y-auto border border-green-200 dark:border-green-800">
            <code>{toolCall.parameters.new_string || 'N/A'}</code>
          </pre>
        </div>
      </div>

      {toolCall.parameters.replace_all && (
        <div className="text-xs text-gray-500">
          Replace all occurrences: Yes
        </div>
      )}

      {/* Result */}
      {toolCall.result && (
        <div className={clsx(
          'flex items-center gap-2 p-2 rounded text-sm',
          toolCall.result.success
            ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400'
            : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400'
        )}>
          {toolCall.result.success ? (
            <>
              <CheckCircledIcon className="w-4 h-4" />
              <span>Edit applied successfully</span>
            </>
          ) : (
            <>
              <CrossCircledIcon className="w-4 h-4" />
              <span>{toolCall.result.error || 'Edit failed'}</span>
            </>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Bash Tool Content
// ============================================================================

function BashToolContent({ toolCall }: ToolCallContentProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    if (toolCall.result?.output) {
      navigator.clipboard.writeText(toolCall.result.output);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="p-3 space-y-3">
      {/* Command */}
      <div className="space-y-1">
        <Label>Command</Label>
        <div className="bg-gray-900 rounded-lg overflow-hidden">
          <div className="flex items-center justify-between px-3 py-1.5 bg-gray-800 border-b border-gray-700">
            <span className="text-xs text-gray-400 font-mono">$</span>
            {toolCall.parameters.description && (
              <span className="text-xs text-gray-500">{toolCall.parameters.description}</span>
            )}
          </div>
          <pre className="p-3 text-sm text-gray-100 overflow-x-auto">
            <code>{toolCall.parameters.command || 'N/A'}</code>
          </pre>
        </div>
      </div>

      {/* Output */}
      {toolCall.result && (
        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <Label>Output</Label>
            <button
              onClick={handleCopy}
              className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            >
              <CopyIcon className="w-3 h-3" />
              {copied ? 'Copied!' : 'Copy'}
            </button>
          </div>
          <pre className="bg-gray-900 text-gray-100 p-3 rounded-lg overflow-x-auto text-xs max-h-64 overflow-y-auto">
            <code>{toolCall.result.output || 'No output'}</code>
          </pre>
        </div>
      )}

      {/* Error */}
      {toolCall.result?.error && (
        <ErrorDisplay error={toolCall.result.error} />
      )}
    </div>
  );
}

// ============================================================================
// Glob Tool Content
// ============================================================================

function GlobToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      <div className="flex gap-4">
        <div className="space-y-1 flex-1">
          <Label>Pattern</Label>
          <CodeBlock>{toolCall.parameters.pattern || 'N/A'}</CodeBlock>
        </div>
        {toolCall.parameters.path && (
          <div className="space-y-1 flex-1">
            <Label>Path</Label>
            <CodeBlock>{toolCall.parameters.path}</CodeBlock>
          </div>
        )}
      </div>

      {/* Result - File list */}
      {toolCall.result?.files && toolCall.result.files.length > 0 && (
        <div className="space-y-1">
          <Label>Matched Files ({toolCall.result.files.length})</Label>
          <div className="bg-gray-50 dark:bg-gray-800 rounded-lg max-h-48 overflow-y-auto">
            {toolCall.result.files.map((file, i) => (
              <div
                key={i}
                className={clsx(
                  'flex items-center gap-2 px-3 py-1.5 text-sm',
                  i !== 0 && 'border-t border-gray-200 dark:border-gray-700'
                )}
              >
                <FileIcon className="w-4 h-4 text-gray-400" />
                <span className="font-mono text-xs truncate">{file}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {toolCall.result && !toolCall.result.files?.length && !toolCall.result.error && (
        <div className="text-sm text-gray-500 italic">No files matched</div>
      )}

      {/* Error */}
      {toolCall.result?.error && (
        <ErrorDisplay error={toolCall.result.error} />
      )}
    </div>
  );
}

// ============================================================================
// Grep Tool Content
// ============================================================================

function GrepToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      <div className="flex gap-4">
        <div className="space-y-1 flex-1">
          <Label>Pattern</Label>
          <CodeBlock>{toolCall.parameters.pattern || 'N/A'}</CodeBlock>
        </div>
        {toolCall.parameters.path && (
          <div className="space-y-1 flex-1">
            <Label>Path</Label>
            <CodeBlock>{toolCall.parameters.path}</CodeBlock>
          </div>
        )}
      </div>

      {/* Result - Matches */}
      {toolCall.result?.matches && toolCall.result.matches.length > 0 && (
        <div className="space-y-1">
          <Label>Matches ({toolCall.result.matches.length})</Label>
          <div className="bg-gray-50 dark:bg-gray-800 rounded-lg max-h-64 overflow-y-auto divide-y divide-gray-200 dark:divide-gray-700">
            {toolCall.result.matches.map((match, i) => (
              <div key={i} className="p-2">
                <div className="flex items-center gap-2 text-xs text-gray-500 mb-1">
                  <FileIcon className="w-3 h-3" />
                  <span className="font-mono truncate">{match.file}</span>
                  <span className="text-gray-400">:</span>
                  <span>{match.line}</span>
                </div>
                <pre className="text-xs font-mono bg-gray-100 dark:bg-gray-900 p-2 rounded overflow-x-auto">
                  {match.content}
                </pre>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Files only mode */}
      {toolCall.result?.files && toolCall.result.files.length > 0 && !toolCall.result.matches && (
        <div className="space-y-1">
          <Label>Files with Matches ({toolCall.result.files.length})</Label>
          <div className="bg-gray-50 dark:bg-gray-800 rounded-lg max-h-48 overflow-y-auto">
            {toolCall.result.files.map((file, i) => (
              <div
                key={i}
                className={clsx(
                  'flex items-center gap-2 px-3 py-1.5 text-sm',
                  i !== 0 && 'border-t border-gray-200 dark:border-gray-700'
                )}
              >
                <FileIcon className="w-4 h-4 text-gray-400" />
                <span className="font-mono text-xs truncate">{file}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {toolCall.result && !toolCall.result.matches?.length && !toolCall.result.files?.length && !toolCall.result.error && (
        <div className="text-sm text-gray-500 italic">No matches found</div>
      )}

      {/* Error */}
      {toolCall.result?.error && (
        <ErrorDisplay error={toolCall.result.error} />
      )}
    </div>
  );
}

// ============================================================================
// Web Tool Content
// ============================================================================

function WebToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      {toolCall.parameters.url && (
        <div className="space-y-1">
          <Label>URL</Label>
          <CodeBlock>{toolCall.parameters.url as string}</CodeBlock>
        </div>
      )}
      {toolCall.parameters.query && (
        <div className="space-y-1">
          <Label>Query</Label>
          <CodeBlock>{toolCall.parameters.query as string}</CodeBlock>
        </div>
      )}

      {/* Result */}
      {toolCall.result?.content && (
        <div className="space-y-1">
          <Label>Result</Label>
          <pre className="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg overflow-x-auto text-xs max-h-64 overflow-y-auto">
            {toolCall.result.content}
          </pre>
        </div>
      )}

      {/* Error */}
      {toolCall.result?.error && (
        <ErrorDisplay error={toolCall.result.error} />
      )}
    </div>
  );
}

// ============================================================================
// Generic Tool Content
// ============================================================================

function GenericToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      <div className="space-y-1">
        <Label>Parameters</Label>
        <pre className="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg overflow-x-auto text-xs">
          <code>{JSON.stringify(toolCall.parameters, null, 2)}</code>
        </pre>
      </div>

      {/* Result */}
      {toolCall.result && (
        <div className="space-y-1">
          <Label>Result</Label>
          <pre className="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg overflow-x-auto text-xs max-h-48 overflow-y-auto">
            <code>{JSON.stringify(toolCall.result, null, 2)}</code>
          </pre>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Shared Components
// ============================================================================

function Label({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
      {children}
    </span>
  );
}

function CodeBlock({ children }: { children: React.ReactNode }) {
  return (
    <code className="block bg-gray-100 dark:bg-gray-800 px-2 py-1 rounded text-sm font-mono text-gray-800 dark:text-gray-200">
      {children}
    </code>
  );
}

function ErrorDisplay({ error }: { error: string }) {
  return (
    <div className="flex items-start gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400">
      <CrossCircledIcon className="w-4 h-4 mt-0.5 flex-shrink-0" />
      <span className="text-sm">{error}</span>
    </div>
  );
}

// ============================================================================
// Helper Functions
// ============================================================================

function getStatusConfig(status: string) {
  switch (status) {
    case 'pending':
      return {
        Icon: ReloadIcon,
        label: 'Pending',
        textColor: 'text-gray-500',
        borderColor: 'border-gray-200 dark:border-gray-700',
        bgColor: 'bg-gray-50 dark:bg-gray-800/50',
      };
    case 'executing':
      return {
        Icon: ReloadIcon,
        label: 'Running',
        textColor: 'text-blue-500',
        borderColor: 'border-blue-200 dark:border-blue-800',
        bgColor: 'bg-blue-50 dark:bg-blue-900/20',
      };
    case 'completed':
      return {
        Icon: CheckCircledIcon,
        label: 'Done',
        textColor: 'text-green-500',
        borderColor: 'border-green-200 dark:border-green-800',
        bgColor: 'bg-green-50 dark:bg-green-900/20',
      };
    case 'failed':
      return {
        Icon: CrossCircledIcon,
        label: 'Failed',
        textColor: 'text-red-500',
        borderColor: 'border-red-200 dark:border-red-800',
        bgColor: 'bg-red-50 dark:bg-red-900/20',
      };
    default:
      return {
        Icon: ReloadIcon,
        label: status,
        textColor: 'text-gray-500',
        borderColor: 'border-gray-200 dark:border-gray-700',
        bgColor: 'bg-gray-50 dark:bg-gray-800/50',
      };
  }
}

function getToolConfig(name: ToolType) {
  switch (name) {
    case 'Read':
      return {
        Icon: FileTextIcon,
        iconBg: 'bg-blue-100 dark:bg-blue-900/50',
        iconColor: 'text-blue-600 dark:text-blue-400',
      };
    case 'Write':
      return {
        Icon: FileTextIcon,
        iconBg: 'bg-green-100 dark:bg-green-900/50',
        iconColor: 'text-green-600 dark:text-green-400',
      };
    case 'Edit':
      return {
        Icon: Pencil1Icon,
        iconBg: 'bg-yellow-100 dark:bg-yellow-900/50',
        iconColor: 'text-yellow-600 dark:text-yellow-400',
      };
    case 'Bash':
      return {
        Icon: TerminalIcon,
        iconBg: 'bg-purple-100 dark:bg-purple-900/50',
        iconColor: 'text-purple-600 dark:text-purple-400',
      };
    case 'Glob':
      return {
        Icon: MagnifyingGlassIcon,
        iconBg: 'bg-orange-100 dark:bg-orange-900/50',
        iconColor: 'text-orange-600 dark:text-orange-400',
      };
    case 'Grep':
      return {
        Icon: MagnifyingGlassIcon,
        iconBg: 'bg-pink-100 dark:bg-pink-900/50',
        iconColor: 'text-pink-600 dark:text-pink-400',
      };
    case 'WebFetch':
    case 'WebSearch':
      return {
        Icon: GlobeIcon,
        iconBg: 'bg-cyan-100 dark:bg-cyan-900/50',
        iconColor: 'text-cyan-600 dark:text-cyan-400',
      };
    default:
      return {
        Icon: FileIcon,
        iconBg: 'bg-gray-100 dark:bg-gray-800',
        iconColor: 'text-gray-600 dark:text-gray-400',
      };
  }
}

function truncatePath(path: string, maxLength = 40): string {
  if (path.length <= maxLength) return path;

  const parts = path.split(/[/\\]/);
  const filename = parts.pop() || '';

  if (filename.length > maxLength - 4) {
    return '...' + filename.slice(-(maxLength - 3));
  }

  let result = filename;
  for (let i = parts.length - 1; i >= 0; i--) {
    const newResult = parts[i] + '/' + result;
    if (newResult.length > maxLength - 4) {
      return '.../' + result;
    }
    result = newResult;
  }

  return result;
}

function truncateCommand(command: string, maxLength = 50): string {
  if (command.length <= maxLength) return command;
  return command.slice(0, maxLength - 3) + '...';
}

export default ToolCallCard;
