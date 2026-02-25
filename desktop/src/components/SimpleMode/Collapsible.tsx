/**
 * Collapsible Component
 *
 * Animates height between 0 and auto using the CSS grid-template-rows trick.
 * Children stay mounted in the DOM (preserving state) regardless of open/closed.
 */

import { clsx } from 'clsx';

interface CollapsibleProps {
  open: boolean;
  children: React.ReactNode;
  className?: string;
  duration?: number;
}

export function Collapsible({ open, children, className, duration = 200 }: CollapsibleProps) {
  return (
    <div
      className={clsx('grid transition-[grid-template-rows] ease-out', className)}
      style={{
        gridTemplateRows: open ? '1fr' : '0fr',
        transitionDuration: `${duration}ms`,
      }}
    >
      <div className="overflow-hidden">{children}</div>
    </div>
  );
}
