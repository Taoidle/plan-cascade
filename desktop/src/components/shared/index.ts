/**
 * Shared Components
 *
 * Exports reusable components that are used across multiple modes.
 *
 * Story 004: Command Palette Enhancement
 * Story 005: Navigation Flow Refinement
 * Story 006: Visual Design Polish - Skeleton component
 * Story 007: Onboarding & Setup Wizard - FeatureTour component
 * Story 008: Real-time Execution Feedback
 */

export {
  GlobalCommandPaletteProvider,
  useGlobalCommandPalette,
  formatShortcut,
  type Command,
  type CommandCategory,
  type CommandGroup,
  type GlobalCommandPaletteContext,
} from './CommandPalette';

export { ContextualActions } from './ContextualActions';
export { ShortcutOverlay } from './ShortcutOverlay';
export { FeatureTour } from './FeatureTour';

export {
  Skeleton,
  SkeletonGroup,
  SettingsSkeleton,
  ListItemSkeleton,
  TableSkeleton,
  MCPServerSkeleton,
  ExpertModeSkeleton,
} from './Skeleton';

export { StreamingOutput } from './StreamingOutput';
export { GlobalProgressBar } from './GlobalProgressBar';
export { QualityGateBadge } from './QualityGateBadge';
export { ErrorState } from './ErrorState';
export { RecoveryPrompt } from './RecoveryPrompt';
export { ProjectSelector } from './ProjectSelector';
export { IndexStatus } from './IndexStatus';
export { DocsIndexStatus } from './DocsIndexStatus';
export { ContextSourceBar } from './ContextSourceBar';
export { KnowledgeSourcePicker } from './KnowledgeSourcePicker';
export { MemorySourcePicker } from './MemorySourcePicker';
export { SkillsSourcePicker } from './SkillsSourcePicker';
