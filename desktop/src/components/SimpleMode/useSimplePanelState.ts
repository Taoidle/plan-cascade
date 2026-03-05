import { useEffect, useState } from 'react';
import type { RightPanelTab } from './TabbedRightPanel';

const RIGHT_PANEL_WIDTH_STORAGE_PREFIX = 'simple_mode_right_panel_width_v1:';
const DEFAULT_RIGHT_PANEL_WIDTH = 620;
const MIN_RIGHT_PANEL_WIDTH = 420;
const MAX_RIGHT_PANEL_WIDTH = 960;

function rightPanelWidthStorageKey(workspacePath: string | null): string {
  return `${RIGHT_PANEL_WIDTH_STORAGE_PREFIX}${workspacePath || '__default_workspace__'}`;
}

export function useSimplePanelState(workspacePath: string | null) {
  const [leftPanelHoverExpanded, setLeftPanelHoverExpanded] = useState(false);
  const [rightPanelHoverExpanded, setRightPanelHoverExpanded] = useState(false);
  const [rightPanelOpen, setRightPanelOpen] = useState(false);
  const [rightPanelWidth, setRightPanelWidth] = useState(DEFAULT_RIGHT_PANEL_WIDTH);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>('output');
  const [supportsPointerHover, setSupportsPointerHover] = useState(false);

  useEffect(() => {
    if (typeof localStorage === 'undefined') return;
    const stored = localStorage.getItem(rightPanelWidthStorageKey(workspacePath));
    if (!stored) return;
    const parsed = Number.parseInt(stored, 10);
    if (!Number.isFinite(parsed)) return;
    setRightPanelWidth(Math.max(MIN_RIGHT_PANEL_WIDTH, Math.min(MAX_RIGHT_PANEL_WIDTH, parsed)));
  }, [workspacePath]);

  useEffect(() => {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(rightPanelWidthStorageKey(workspacePath), String(Math.round(rightPanelWidth)));
  }, [workspacePath, rightPanelWidth]);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const media = window.matchMedia('(hover: hover) and (pointer: fine)');
    const handleChange = () => setSupportsPointerHover(media.matches);
    handleChange();
    media.addEventListener('change', handleChange);
    return () => media.removeEventListener('change', handleChange);
  }, []);

  return {
    leftPanelHoverExpanded,
    rightPanelHoverExpanded,
    rightPanelOpen,
    rightPanelWidth,
    rightPanelTab,
    supportsPointerHover,
    setLeftPanelHoverExpanded,
    setRightPanelHoverExpanded,
    setRightPanelOpen,
    setRightPanelWidth,
    setRightPanelTab,
  };
}
