/**
 * ClaudeCodeMode Components
 *
 * Re-export all Claude Code mode components for easier imports.
 */

export { ClaudeCodeMode } from './index';
export { ChatView } from './ChatView';
export { ChatInput } from './ChatInput';
export { ToolCallCard } from './ToolCallCard';
export { ToolHistorySidebar } from './ToolHistorySidebar';
export { ExportDialog } from './ExportDialog';

// Story 011-1: Enhanced Markdown Rendering
export { MarkdownRenderer } from './MarkdownRenderer';

// Story 011-2: Code Block Actions
export { CodeBlock, SimpleCodeBlock } from './CodeBlock';

// Story 011-3: File Attachment and @ File References
export {
  FileAttachmentDropZone,
  FileChip,
  FileReferenceAutocomplete,
  useFileReferences,
} from './FileAttachment';

// Story 011-4: Message Actions
export { MessageActions, useMessageActions } from './MessageActions';

// Story 011-5: Keyboard Shortcuts
export {
  ShortcutsHelpDialog,
  KeyboardShortcutHint,
  ShortcutProvider,
  useShortcutRegistry,
  useKeyboardShortcut,
  useChatShortcuts,
  formatShortcut,
  getPlatformModifier,
  DEFAULT_SHORTCUTS,
} from './KeyboardShortcuts';

// Story 011-6: Session Control
export {
  SessionControlProvider,
  SessionControlBar,
  SessionStateIndicator,
  InlineSessionControl,
  useSessionControl,
  useStreamingWithControl,
} from './SessionControl';

// Story 011-7: Command Palette
export {
  CommandPaletteProvider,
  useCommandPalette,
  createDefaultCommands,
  useRegisterCommands,
} from './CommandPalette';

// New components for Tool Call Visualization Enhancement (feature-012)
export { TruncatedText, useExpandable, truncatePath, truncateCommand, truncateAtWordBoundary } from './TruncatedText';
export { AnsiOutput, parseAnsiText, get256Color } from './AnsiOutput';
export { GlobResultViewer, getFileIcon, buildFileTree } from './GlobResultViewer';
export { GrepResultViewer, highlightPattern } from './GrepResultViewer';
export { EnhancedDiffViewer, computeDiff, computeCharDiff } from './EnhancedDiffViewer';
export { ExecutionTimeline, calculateTimeline, calculateStatistics, TOOL_COLORS } from './ExecutionTimeline';
