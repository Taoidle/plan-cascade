/**
 * MarkdownFallback Component
 *
 * Fallback renderer for unknown rich content types.
 * Renders the data as pretty-printed JSON in a code block.
 *
 * Story 002: DynamicRenderer frontend component
 */

import { clsx } from 'clsx';

interface MarkdownFallbackProps {
  componentType: string;
  data: unknown;
}

export function MarkdownFallback({ componentType, data }: MarkdownFallbackProps) {
  const jsonStr = JSON.stringify(data, null, 2);

  return (
    <div className="space-y-2" data-testid="markdown-fallback">
      <div className="flex items-center gap-2 text-xs">
        <span className="px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 font-medium">
          {componentType}
        </span>
        <span className="text-gray-400 dark:text-gray-500">(unknown component type)</span>
      </div>
      <pre
        className={clsx(
          'overflow-x-auto p-3 rounded-lg text-xs',
          'bg-gray-50 dark:bg-gray-900',
          'border border-gray-200 dark:border-gray-700',
          'text-gray-700 dark:text-gray-300',
          'font-mono',
        )}
      >
        {jsonStr}
      </pre>
    </div>
  );
}

export default MarkdownFallback;
