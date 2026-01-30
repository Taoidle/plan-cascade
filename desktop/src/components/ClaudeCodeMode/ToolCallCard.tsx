/**
 * ToolCallCard Component
 *
 * Visualizes tool calls with collapsible sections showing parameters and results.
 * Each tool type has distinct visual styling with enhanced state visualization,
 * animated transitions, and revert functionality for file-changing operations.
 *
 * Story-001: ToolCallCard state display and styling enhancement
 * Story-002: Argument preview with truncation (via TruncatedText)
 * Story-003: File diff viewer (via EnhancedDiffViewer)
 * Story-004: ANSI output support (via AnsiOutput)
 * Story-005: Glob/Grep viewers
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import {
  FileTextIcon,
  Pencil1Icon,
  CodeIcon,
  MagnifyingGlassIcon,
  FileIcon,
  CheckCircledIcon,
  CrossCircledIcon,
  ReloadIcon,
  ChevronDownIcon,
  CopyIcon,
  GlobeIcon,
  ClockIcon,
  ResetIcon,
  ExclamationTriangleIcon,
} from '@radix-ui/react-icons';
import { ToolCall, ToolType } from '../../store/claudeCode';
import { TruncatedText } from './TruncatedText';
import { AnsiOutput } from './AnsiOutput';
import { GlobResultViewer } from './GlobResultViewer';
import { GrepResultViewer } from './GrepResultViewer';
import { EnhancedDiffViewer } from './EnhancedDiffViewer';

// ============================================================================
// Custom Hook: useToolCallState
// ============================================================================

interface UseToolCallStateOptions {
  toolCall: ToolCall;
}

function useToolCallState({ toolCall }: UseToolCallStateOptions) {
  const [elapsedTime, setElapsedTime] = useState(0);
  const [showRevertConfirm, setShowRevertConfirm] = useState(false);

  // Update elapsed time for running operations
  useEffect(() => {
    if (toolCall.status === 'executing' && toolCall.startedAt) {
      const startTime = new Date(toolCall.startedAt).getTime();

      const updateTimer = () => {
        setElapsedTime(Date.now() - startTime);
      };

      updateTimer();
      const interval = setInterval(updateTimer, 100);

      return () => clearInterval(interval);
    } else if (toolCall.duration) {
      setElapsedTime(toolCall.duration);
    }
  }, [toolCall.status, toolCall.startedAt, toolCall.duration]);

  return {
    elapsedTime,
    showRevertConfirm,
    setShowRevertConfirm,
  };
}

// ============================================================================
// RevertButton Component
// ============================================================================

interface RevertButtonProps {
  toolCall: ToolCall;
  showConfirm: boolean;
  setShowConfirm: (show: boolean) => void;
}

function RevertButton({ toolCall, showConfirm, setShowConfirm }: RevertButtonProps) {
  const handleRevert = useCallback(() => {
    // In a real implementation, this would trigger the revert action
    // through the claudeCode store or a dedicated API
    console.log('Reverting tool call:', toolCall.id);
    setShowConfirm(false);
    // TODO: Integrate with checkpoint/restore system
  }, [toolCall.id, setShowConfirm]);

  if (!showConfirm) {
    return (
      <button
        onClick={(e) => {
          e.stopPropagation();
          setShowConfirm(true);
        }}
        className={clsx(
          'flex items-center gap-1 px-2 py-1 rounded text-xs',
          'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-400',
          'hover:bg-yellow-200 dark:hover:bg-yellow-900/50',
          'transition-colors'
        )}
        title="Revert this change"
      >
        <ResetIcon className="w-3 h-3" />
        Revert
      </button>
    );
  }

  return (
    <div className="flex items-center gap-2" onClick={(e) => e.stopPropagation()}>
      <span className="text-xs text-gray-500">Confirm revert?</span>
      <button
        onClick={handleRevert}
        className={clsx(
          'px-2 py-1 rounded text-xs',
          'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400',
          'hover:bg-red-200 dark:hover:bg-red-900/50',
          'transition-colors'
        )}
      >
        Yes
      </button>
      <button
        onClick={() => setShowConfirm(false)}
        className={clsx(
          'px-2 py-1 rounded text-xs',
          'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
          'hover:bg-gray-200 dark:hover:bg-gray-700',
          'transition-colors'
        )}
      >
        No
      </button>
    </div>
  );
}

// ============================================================================
// ToolCallCard Component
// ============================================================================

interface ToolCallCardProps {
  toolCall: ToolCall;
  compact?: boolean;
}

export function ToolCallCard({ toolCall, compact = false }: ToolCallCardProps) {
  const [isExpanded, setIsExpanded] = useState(!compact);
  const { elapsedTime, showRevertConfirm, setShowRevertConfirm } = useToolCallState({ toolCall });

  const statusConfig = getStatusConfig(toolCall.status);
  const toolConfig = getToolConfig(toolCall.name);

  // Check if this is a file-changing operation that can be reverted
  const canRevert = useMemo(() => {
    return (
      (toolCall.name === 'Write' || toolCall.name === 'Edit') &&
      toolCall.status === 'completed' &&
      toolCall.result?.success
    );
  }, [toolCall.name, toolCall.status, toolCall.result?.success]);

  return (
    <div
      id={`tool-call-${toolCall.id}`}
      className={clsx(
        'rounded-lg border overflow-hidden',
        'transition-all duration-300 ease-in-out',
        statusConfig.borderColor,
        statusConfig.bgColor,
        // Running state gets animated border
        toolCall.status === 'executing' && 'animate-pulse-border'
      )}
    >
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={clsx(
          'w-full flex items-center gap-3 px-3 py-2',
          'hover:bg-black/5 dark:hover:bg-white/5',
          'transition-colors text-left',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-inset'
        )}
        aria-expanded={isExpanded}
        aria-label={`${toolCall.name} tool call, status: ${statusConfig.label}`}
      >
        {/* Expand/Collapse icon */}
        <span className="text-gray-400 transition-transform duration-200" style={{
          transform: isExpanded ? 'rotate(0deg)' : 'rotate(-90deg)'
        }}>
          <ChevronDownIcon className="w-4 h-4" />
        </span>

        {/* Tool icon with status-based animation */}
        <span className={clsx(
          'p-1.5 rounded transition-all duration-300',
          toolConfig.iconBg,
          toolCall.status === 'executing' && 'animate-pulse'
        )}>
          <toolConfig.Icon className={clsx('w-4 h-4', toolConfig.iconColor)} />
        </span>

        {/* Tool name and summary */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm text-gray-900 dark:text-white">
              {toolCall.name}
            </span>
            {toolCall.parameters.file_path && (
              <span className="text-xs text-gray-500 dark:text-gray-400 truncate max-w-[200px]">
                {truncatePath(toolCall.parameters.file_path)}
              </span>
            )}
            {toolCall.parameters.command && (
              <span className="text-xs text-gray-500 dark:text-gray-400 truncate font-mono max-w-[200px]">
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

        {/* Duration timer */}
        {(toolCall.status === 'executing' || toolCall.duration) && (
          <span className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
            <ClockIcon className="w-3 h-3" />
            {formatDuration(elapsedTime)}
          </span>
        )}

        {/* Status indicator */}
        <span className={clsx(
          'flex items-center gap-1.5 px-2 py-0.5 rounded-full',
          statusConfig.badgeBg,
          statusConfig.textColor,
          'transition-all duration-300'
        )}>
          <statusConfig.Icon
            className={clsx(
              'w-3.5 h-3.5',
              toolCall.status === 'executing' && 'animate-spin'
            )}
          />
          <span className="text-xs font-medium">{statusConfig.label}</span>
        </span>
      </button>

      {/* Revert button for file operations */}
      {canRevert && !isExpanded && (
        <div className="px-3 pb-2">
          <RevertButton
            toolCall={toolCall}
            showConfirm={showRevertConfirm}
            setShowConfirm={setShowRevertConfirm}
          />
        </div>
      )}

      {/* Expanded content */}
      <div
        className={clsx(
          'overflow-hidden transition-all duration-300 ease-in-out',
          isExpanded ? 'max-h-[2000px] opacity-100' : 'max-h-0 opacity-0'
        )}
      >
        <div className="border-t border-gray-200 dark:border-gray-700">
          <ToolCallContent toolCall={toolCall} />

          {/* Revert button at bottom when expanded */}
          {canRevert && (
            <div className="px-3 py-2 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700">
              <RevertButton
                toolCall={toolCall}
                showConfirm={showRevertConfirm}
                setShowConfirm={setShowRevertConfirm}
              />
            </div>
          )}
        </div>
      </div>

      {/* CSS for animated border */}
      <style>{`
        @keyframes pulse-border {
          0%, 100% {
            border-color: rgb(59 130 246 / 0.5);
            box-shadow: 0 0 0 0 rgb(59 130 246 / 0.4);
          }
          50% {
            border-color: rgb(59 130 246 / 1);
            box-shadow: 0 0 8px 2px rgb(59 130 246 / 0.2);
          }
        }
        .animate-pulse-border {
          animation: pulse-border 2s ease-in-out infinite;
        }
      `}</style>
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
      <div className="space-y-2">
        <TruncatedText
          content={toolCall.parameters.file_path || 'N/A'}
          isPath
          label="File Path"
          maxLength={80}
        />
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
          <TruncatedText
            content={toolCall.result.content || toolCall.result.output || 'No content'}
            maxLength={500}
            maxExpandedHeight={400}
          />
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
      <TruncatedText
        content={toolCall.parameters.file_path || 'N/A'}
        isPath
        label="File Path"
        maxLength={80}
      />

      <div className="space-y-1">
        <Label>Content</Label>
        <TruncatedText
          content={toolCall.parameters.content || 'No content'}
          maxLength={300}
          maxExpandedHeight={400}
        />
      </div>

      {/* Result */}
      {toolCall.result && (
        <div className={clsx(
          'flex items-center gap-2 p-2 rounded text-sm',
          'transition-all duration-300',
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
      <TruncatedText
        content={toolCall.parameters.file_path || 'N/A'}
        isPath
        label="File Path"
        maxLength={80}
      />

      {/* Enhanced Diff view */}
      {toolCall.parameters.old_string && toolCall.parameters.new_string && (
        <EnhancedDiffViewer
          oldContent={toolCall.parameters.old_string}
          newContent={toolCall.parameters.new_string}
          filePath={toolCall.parameters.file_path}
          maxHeight={300}
        />
      )}

      {toolCall.parameters.replace_all && (
        <div className="text-xs text-gray-500 flex items-center gap-1">
          <ExclamationTriangleIcon className="w-3 h-3" />
          Replace all occurrences: Yes
        </div>
      )}

      {/* Result */}
      {toolCall.result && (
        <div className={clsx(
          'flex items-center gap-2 p-2 rounded text-sm',
          'transition-all duration-300',
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
          <TruncatedText
            content={toolCall.parameters.command || 'N/A'}
            isCommand
            maxLength={150}
            className="p-0"
          />
        </div>
      </div>

      {/* Output with ANSI support */}
      {toolCall.result && (
        <AnsiOutput
          output={toolCall.result.output || 'No output'}
          exitCode={toolCall.result.success === false ? 1 : 0}
          duration={toolCall.duration}
          maxHeight={300}
        />
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
        <TruncatedText
          content={toolCall.parameters.pattern || 'N/A'}
          label="Pattern"
          maxLength={60}
          className="flex-1"
        />
        {toolCall.parameters.path && (
          <TruncatedText
            content={toolCall.parameters.path}
            isPath
            label="Path"
            maxLength={60}
            className="flex-1"
          />
        )}
      </div>

      {/* Result - Enhanced File list */}
      {toolCall.result?.files && toolCall.result.files.length > 0 && (
        <GlobResultViewer
          files={toolCall.result.files}
          pattern={toolCall.parameters.pattern}
          maxHeight={300}
        />
      )}

      {toolCall.result && !toolCall.result.files?.length && !toolCall.result.error && (
        <div className="text-sm text-gray-500 italic p-4 text-center bg-gray-50 dark:bg-gray-800/50 rounded-lg">
          No files matched
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
// Grep Tool Content
// ============================================================================

function GrepToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      <div className="flex gap-4">
        <TruncatedText
          content={toolCall.parameters.pattern || 'N/A'}
          label="Pattern"
          maxLength={60}
          className="flex-1"
        />
        {toolCall.parameters.path && (
          <TruncatedText
            content={toolCall.parameters.path}
            isPath
            label="Path"
            maxLength={60}
            className="flex-1"
          />
        )}
      </div>

      {/* Result - Enhanced Matches viewer */}
      {toolCall.result?.matches && toolCall.result.matches.length > 0 && (
        <GrepResultViewer
          matches={toolCall.result.matches}
          pattern={toolCall.parameters.pattern}
          outputMode="content"
          maxHeight={400}
        />
      )}

      {/* Files only mode */}
      {toolCall.result?.files && toolCall.result.files.length > 0 && !toolCall.result.matches && (
        <GrepResultViewer
          matches={[]}
          files={toolCall.result.files}
          pattern={toolCall.parameters.pattern}
          outputMode="files_with_matches"
          maxHeight={300}
        />
      )}

      {toolCall.result && !toolCall.result.matches?.length && !toolCall.result.files?.length && !toolCall.result.error && (
        <div className="text-sm text-gray-500 italic p-4 text-center bg-gray-50 dark:bg-gray-800/50 rounded-lg">
          No matches found
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
// Web Tool Content
// ============================================================================

function WebToolContent({ toolCall }: ToolCallContentProps) {
  return (
    <div className="p-3 space-y-3">
      {/* Parameters */}
      {!!toolCall.parameters.url && (
        <TruncatedText
          content={toolCall.parameters.url as string}
          label="URL"
          maxLength={80}
        />
      )}
      {!!toolCall.parameters.query && (
        <TruncatedText
          content={toolCall.parameters.query as string}
          label="Query"
          maxLength={100}
        />
      )}

      {/* Result */}
      {toolCall.result?.content && (
        <div className="space-y-1">
          <Label>Result</Label>
          <TruncatedText
            content={toolCall.result.content}
            maxLength={500}
            maxExpandedHeight={400}
          />
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
        <TruncatedText
          content={JSON.stringify(toolCall.parameters, null, 2)}
          isJson
          maxLength={300}
        />
      </div>

      {/* Result */}
      {toolCall.result && (
        <div className="space-y-1">
          <Label>Result</Label>
          <TruncatedText
            content={JSON.stringify(toolCall.result, null, 2)}
            isJson
            maxLength={300}
          />
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
        Icon: ClockIcon,
        label: 'Pending',
        textColor: 'text-gray-600 dark:text-gray-400',
        borderColor: 'border-gray-300 dark:border-gray-600',
        bgColor: 'bg-gray-50 dark:bg-gray-800/50',
        badgeBg: 'bg-gray-200 dark:bg-gray-700',
      };
    case 'executing':
      return {
        Icon: ReloadIcon,
        label: 'Running',
        textColor: 'text-blue-600 dark:text-blue-400',
        borderColor: 'border-blue-300 dark:border-blue-700',
        bgColor: 'bg-blue-50 dark:bg-blue-900/20',
        badgeBg: 'bg-blue-100 dark:bg-blue-900/50',
      };
    case 'completed':
      return {
        Icon: CheckCircledIcon,
        label: 'Done',
        textColor: 'text-green-600 dark:text-green-400',
        borderColor: 'border-green-300 dark:border-green-700',
        bgColor: 'bg-green-50 dark:bg-green-900/20',
        badgeBg: 'bg-green-100 dark:bg-green-900/50',
      };
    case 'failed':
      return {
        Icon: CrossCircledIcon,
        label: 'Failed',
        textColor: 'text-red-600 dark:text-red-400',
        borderColor: 'border-red-300 dark:border-red-700',
        bgColor: 'bg-red-50 dark:bg-red-900/20',
        badgeBg: 'bg-red-100 dark:bg-red-900/50',
      };
    default:
      return {
        Icon: ReloadIcon,
        label: status,
        textColor: 'text-gray-600 dark:text-gray-400',
        borderColor: 'border-gray-300 dark:border-gray-600',
        bgColor: 'bg-gray-50 dark:bg-gray-800/50',
        badgeBg: 'bg-gray-200 dark:bg-gray-700',
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
        Icon: CodeIcon,
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

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

export default ToolCallCard;
