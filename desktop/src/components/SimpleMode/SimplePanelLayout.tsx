import { clsx } from 'clsx';
import type { ReactNode } from 'react';

interface SimplePanelLayoutProps {
  hoverPanelsEnabled: boolean;
  isLeftPanelOpen: boolean;
  isRightPanelOpen: boolean;
  rightPanelWidth: number;
  leftPanel: ReactNode;
  middlePanel: ReactNode;
  rightPanel: ReactNode;
  onLeftEdgeEnter: () => void;
  onLeftEdgeLeave: () => void;
  onRightEdgeEnter: () => void;
  onRightEdgeLeave: () => void;
}

export function SimplePanelLayout({
  hoverPanelsEnabled,
  isLeftPanelOpen,
  isRightPanelOpen,
  rightPanelWidth,
  leftPanel,
  middlePanel,
  rightPanel,
  onLeftEdgeEnter,
  onLeftEdgeLeave,
  onRightEdgeEnter,
  onRightEdgeLeave,
}: SimplePanelLayoutProps) {
  return (
    <div className="flex-1 min-h-0 px-4 py-2">
      <div className="relative h-full max-w-[2200px] mx-auto w-full flex">
        {hoverPanelsEnabled && (
          <>
            <div
              className="absolute left-0 top-0 bottom-0 w-2 z-20"
              onMouseEnter={onLeftEdgeEnter}
              onMouseLeave={onLeftEdgeLeave}
            />
            <div
              className="absolute right-0 top-0 bottom-0 w-2 z-20"
              onMouseEnter={onRightEdgeEnter}
              onMouseLeave={onRightEdgeLeave}
            />
          </>
        )}

        <div
          className={clsx(
            'shrink-0 transition-all duration-200 ease-out overflow-hidden',
            isLeftPanelOpen ? 'w-[280px] opacity-100 mr-3' : 'w-0 opacity-0',
          )}
          onMouseEnter={onLeftEdgeEnter}
          onMouseLeave={onLeftEdgeLeave}
        >
          <div className="w-[280px] h-full">{leftPanel}</div>
        </div>

        {middlePanel}

        <div
          className={clsx(
            'relative shrink-0 transition-[width,opacity,margin] duration-200 ease-out overflow-hidden',
            isRightPanelOpen ? 'opacity-100 ml-3' : 'opacity-0 ml-0',
          )}
          style={{ width: isRightPanelOpen ? rightPanelWidth : 0 }}
          onMouseEnter={onRightEdgeEnter}
          onMouseLeave={onRightEdgeLeave}
        >
          {rightPanel}
        </div>
      </div>
    </div>
  );
}

export default SimplePanelLayout;
