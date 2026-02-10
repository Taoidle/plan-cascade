/**
 * Skeleton Component
 *
 * Reusable skeleton loading component with shimmer animation.
 * Provides consistent loading states across the application.
 *
 * Story 006: Visual Design Polish
 *
 * Usage:
 *   <Skeleton width="100%" height="1rem" />
 *   <Skeleton variant="circle" size="2.5rem" />
 *   <Skeleton variant="text" lines={3} />
 *   <Skeleton variant="card" />
 *   <SkeletonGroup count={3}>{(i) => <Skeleton key={i} />}</SkeletonGroup>
 */

import { clsx } from 'clsx';

// ============================================================================
// Types
// ============================================================================

interface SkeletonBaseProps {
  className?: string;
  /** Animation style */
  animate?: boolean;
}

interface SkeletonRectProps extends SkeletonBaseProps {
  variant?: 'rect';
  /** Width - CSS value or Tailwind class */
  width?: string;
  /** Height - CSS value or Tailwind class */
  height?: string;
  /** Border radius */
  rounded?: 'sm' | 'md' | 'lg' | 'xl' | 'full' | 'none';
}

interface SkeletonCircleProps extends SkeletonBaseProps {
  variant: 'circle';
  /** Diameter */
  size?: string;
}

interface SkeletonTextProps extends SkeletonBaseProps {
  variant: 'text';
  /** Number of text lines */
  lines?: number;
  /** Gap between lines */
  gap?: string;
  /** Make last line shorter */
  lastLineWidth?: string;
}

interface SkeletonCardProps extends SkeletonBaseProps {
  variant: 'card';
  /** Show image area */
  showImage?: boolean;
  /** Number of text lines in card body */
  lines?: number;
}

interface SkeletonButtonProps extends SkeletonBaseProps {
  variant: 'button';
  /** Button size */
  size?: 'sm' | 'md' | 'lg';
}

interface SkeletonAvatarProps extends SkeletonBaseProps {
  variant: 'avatar';
  /** Avatar size */
  size?: 'sm' | 'md' | 'lg';
}

interface SkeletonBadgeProps extends SkeletonBaseProps {
  variant: 'badge';
}

type SkeletonProps =
  | SkeletonRectProps
  | SkeletonCircleProps
  | SkeletonTextProps
  | SkeletonCardProps
  | SkeletonButtonProps
  | SkeletonAvatarProps
  | SkeletonBadgeProps;

// ============================================================================
// Base Skeleton Element
// ============================================================================

function SkeletonBase({
  className,
  style,
  animate = true,
}: {
  className?: string;
  style?: React.CSSProperties;
  animate?: boolean;
}) {
  return (
    <div
      className={clsx(
        'bg-gray-200 dark:bg-gray-700',
        animate && 'animate-skeleton',
        className
      )}
      style={style}
      aria-hidden="true"
    />
  );
}

// ============================================================================
// Skeleton Component
// ============================================================================

export function Skeleton(props: SkeletonProps) {
  const { variant = 'rect', className, animate = true } = props;

  switch (variant) {
    case 'circle': {
      const { size = '2.5rem' } = props as SkeletonCircleProps;
      return (
        <SkeletonBase
          className={clsx('rounded-full shrink-0', className)}
          style={{ width: size, height: size }}
          animate={animate}
        />
      );
    }

    case 'text': {
      const {
        lines = 3,
        gap = '0.5rem',
        lastLineWidth = '60%',
      } = props as SkeletonTextProps;
      return (
        <div
          className={clsx('w-full', className)}
          style={{ display: 'flex', flexDirection: 'column', gap }}
          aria-hidden="true"
        >
          {Array.from({ length: lines }).map((_, i) => (
            <SkeletonBase
              key={i}
              className="h-4 rounded"
              style={{
                width: i === lines - 1 && lines > 1 ? lastLineWidth : '100%',
              }}
              animate={animate}
            />
          ))}
        </div>
      );
    }

    case 'card': {
      const { showImage = false, lines = 2 } = props as SkeletonCardProps;
      return (
        <div
          className={clsx(
            'rounded-xl border border-gray-200 dark:border-gray-700 overflow-hidden',
            className
          )}
          aria-hidden="true"
        >
          {showImage && (
            <SkeletonBase
              className="w-full h-40"
              animate={animate}
            />
          )}
          <div className="p-4 space-y-3">
            <SkeletonBase className="h-5 w-2/3 rounded" animate={animate} />
            {Array.from({ length: lines }).map((_, i) => (
              <SkeletonBase
                key={i}
                className="h-4 rounded"
                style={{ width: i === lines - 1 ? '80%' : '100%' }}
                animate={animate}
              />
            ))}
          </div>
        </div>
      );
    }

    case 'button': {
      const { size = 'md' } = props as SkeletonButtonProps;
      const sizeClasses = {
        sm: 'h-8 w-20',
        md: 'h-10 w-24',
        lg: 'h-12 w-32',
      };
      return (
        <SkeletonBase
          className={clsx('rounded-lg', sizeClasses[size], className)}
          animate={animate}
        />
      );
    }

    case 'avatar': {
      const { size = 'md' } = props as SkeletonAvatarProps;
      const sizeMap = {
        sm: '2rem',
        md: '2.5rem',
        lg: '3rem',
      };
      return (
        <SkeletonBase
          className={clsx('rounded-full shrink-0', className)}
          style={{ width: sizeMap[size], height: sizeMap[size] }}
          animate={animate}
        />
      );
    }

    case 'badge': {
      return (
        <SkeletonBase
          className={clsx('h-5 w-16 rounded-full', className)}
          animate={animate}
        />
      );
    }

    case 'rect':
    default: {
      const {
        width = '100%',
        height = '1rem',
        rounded = 'md',
      } = props as SkeletonRectProps;
      const roundedClasses = {
        none: '',
        sm: 'rounded-sm',
        md: 'rounded',
        lg: 'rounded-lg',
        xl: 'rounded-xl',
        full: 'rounded-full',
      };
      return (
        <SkeletonBase
          className={clsx(roundedClasses[rounded], className)}
          style={{ width, height }}
          animate={animate}
        />
      );
    }
  }
}

// ============================================================================
// SkeletonGroup - Renders multiple skeleton items
// ============================================================================

interface SkeletonGroupProps {
  /** Number of skeleton items */
  count: number;
  /** Render function for each skeleton */
  children: (index: number) => React.ReactNode;
  /** Container className */
  className?: string;
}

export function SkeletonGroup({ count, children, className }: SkeletonGroupProps) {
  return (
    <div className={className} aria-hidden="true">
      {Array.from({ length: count }).map((_, i) => children(i))}
    </div>
  );
}

// ============================================================================
// Pre-built Skeleton Compositions
// ============================================================================

/** Skeleton for a settings section with label and input */
export function SettingsSkeleton() {
  return (
    <div className="space-y-6" aria-hidden="true">
      {Array.from({ length: 3 }).map((_, i) => (
        <div key={i} className="space-y-2">
          <Skeleton width="30%" height="1rem" />
          <Skeleton width="100%" height="2.5rem" rounded="lg" />
          <Skeleton width="60%" height="0.75rem" />
        </div>
      ))}
    </div>
  );
}

/** Skeleton for a list item with icon and text */
export function ListItemSkeleton() {
  return (
    <div className="flex items-center gap-3 p-3" aria-hidden="true">
      <Skeleton variant="circle" size="2rem" />
      <div className="flex-1 space-y-2">
        <Skeleton width="40%" height="1rem" />
        <Skeleton width="70%" height="0.75rem" />
      </div>
      <Skeleton variant="badge" />
    </div>
  );
}

/** Skeleton for a table with header and rows */
export function TableSkeleton({ rows = 5, cols = 4 }: { rows?: number; cols?: number }) {
  return (
    <div className="space-y-3" aria-hidden="true">
      {/* Header */}
      <div className="flex gap-4 pb-3 border-b border-gray-200 dark:border-gray-700">
        {Array.from({ length: cols }).map((_, i) => (
          <Skeleton key={i} width={i === 0 ? '25%' : '18%'} height="0.875rem" />
        ))}
      </div>
      {/* Rows */}
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex gap-4 py-2">
          {Array.from({ length: cols }).map((_, j) => (
            <Skeleton key={j} width={j === 0 ? '25%' : '18%'} height="0.875rem" />
          ))}
        </div>
      ))}
    </div>
  );
}

/** Skeleton for the MCP server card */
export function MCPServerSkeleton() {
  return (
    <div
      className="p-4 rounded-lg border border-gray-200 dark:border-gray-700 animate-pulse"
      aria-hidden="true"
    >
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="w-2.5 h-2.5 rounded-full bg-gray-200 dark:bg-gray-700" />
          <div>
            <Skeleton width="8rem" height="1.25rem" className="mb-1" />
            <div className="flex gap-2">
              <Skeleton variant="badge" />
              <Skeleton width="4rem" height="0.875rem" />
            </div>
          </div>
        </div>
        <Skeleton width="2.25rem" height="1.25rem" rounded="full" />
      </div>
      <Skeleton width="12rem" height="0.875rem" className="mb-3" />
      <div className="flex gap-2">
        <Skeleton variant="button" size="sm" />
        <Skeleton variant="button" size="sm" />
      </div>
    </div>
  );
}

/** Skeleton for Expert mode PRD form */
export function ExpertModeSkeleton() {
  return (
    <div className="max-w-2xl mx-auto space-y-6 p-6" aria-hidden="true">
      <div className="space-y-2">
        <Skeleton width="50%" height="1.5rem" />
        <Skeleton width="80%" height="1rem" />
      </div>
      <Skeleton width="100%" height="10rem" rounded="xl" />
      <div className="grid grid-cols-3 gap-3">
        <Skeleton height="5rem" rounded="lg" />
        <Skeleton height="5rem" rounded="lg" />
        <Skeleton height="5rem" rounded="lg" />
      </div>
      <Skeleton variant="button" size="lg" className="w-full" />
    </div>
  );
}

export default Skeleton;
