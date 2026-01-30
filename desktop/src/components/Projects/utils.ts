/**
 * Project Browser Utilities
 *
 * Helper functions for formatting and display.
 */

/**
 * Format a timestamp as relative time (e.g., "2h ago", "Yesterday", "Jan 28")
 */
export function formatRelativeTime(timestamp: string): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) {
    return 'Just now';
  }
  if (diffMins < 60) {
    return `${diffMins}m ago`;
  }
  if (diffHours < 24) {
    return `${diffHours}h ago`;
  }
  if (diffDays === 1) {
    return 'Yesterday';
  }
  if (diffDays < 7) {
    return `${diffDays}d ago`;
  }

  // Format as month day
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

/**
 * Truncate a file path for display, keeping the end visible
 */
export function truncatePath(path: string, maxLength: number): string {
  if (path.length <= maxLength) {
    return path;
  }

  // Replace home directory with ~
  const homePath = path.replace(/^(C:\\Users\\[^\\]+|\/home\/[^/]+|\/Users\/[^/]+)/, '~');

  if (homePath.length <= maxLength) {
    return homePath;
  }

  // Truncate from the beginning
  return '...' + homePath.slice(-(maxLength - 3));
}

/**
 * Truncate text to a maximum length with ellipsis
 */
export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) {
    return text;
  }
  return text.slice(0, maxLength - 3) + '...';
}

/**
 * Debounce a function call
 */
export function debounce<Args extends unknown[]>(
  func: (...args: Args) => void,
  wait: number
): (...args: Args) => void {
  let timeout: ReturnType<typeof setTimeout> | null = null;

  return (...args: Args) => {
    if (timeout) {
      clearTimeout(timeout);
    }
    timeout = setTimeout(() => {
      func(...args);
    }, wait);
  };
}
