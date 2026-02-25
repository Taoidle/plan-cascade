/**
 * GraphNodeComponent
 *
 * Renders a single graph node as a draggable div with absolute positioning.
 * Shows the agent step type and name.
 */

import { useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { GraphNode, NodePosition } from '../../types/graphWorkflow';

interface GraphNodeComponentProps {
  node: GraphNode;
  isSelected: boolean;
  isEntryNode: boolean;
  onClick: () => void;
  onDrag: (position: NodePosition) => void;
}

const STEP_TYPE_COLORS: Record<string, string> = {
  llm_step: 'border-blue-400 dark:border-blue-600',
  sequential_step: 'border-green-400 dark:border-green-600',
  parallel_step: 'border-purple-400 dark:border-purple-600',
  conditional_step: 'border-amber-400 dark:border-amber-600',
};

const STEP_TYPE_LABEL_KEYS: Record<string, string> = {
  llm_step: 'graphWorkflow.nodeTypes.llm',
  sequential_step: 'graphWorkflow.nodeTypes.sequential',
  parallel_step: 'graphWorkflow.nodeTypes.parallel',
  conditional_step: 'graphWorkflow.nodeTypes.conditional',
};

export function GraphNodeComponent({ node, isSelected, isEntryNode, onClick, onDrag }: GraphNodeComponentProps) {
  const { t } = useTranslation('expertMode');
  const nodeRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const dragStart = useRef<{ x: number; y: number } | null>(null);

  const pos = node.position ?? { x: 100, y: 100 };
  const stepType = node.agent_step.step_type;
  const stepName = node.agent_step.name;

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return; // Left click only
      e.stopPropagation();
      setIsDragging(true);
      dragStart.current = { x: e.clientX - pos.x, y: e.clientY - pos.y };

      const handleMouseMove = (ev: MouseEvent) => {
        if (dragStart.current) {
          const newX = ev.clientX - dragStart.current.x;
          const newY = ev.clientY - dragStart.current.y;
          onDrag({ x: Math.max(0, newX), y: Math.max(0, newY) });
        }
      };

      const handleMouseUp = () => {
        setIsDragging(false);
        dragStart.current = null;
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };

      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    },
    [pos.x, pos.y, onDrag],
  );

  return (
    <div
      ref={nodeRef}
      className={clsx(
        'absolute px-4 py-3 rounded-lg border-2 cursor-pointer transition-shadow select-none',
        'bg-white dark:bg-gray-900 shadow-sm',
        'min-w-[140px]',
        STEP_TYPE_COLORS[stepType] ?? 'border-gray-400',
        isSelected && 'ring-2 ring-primary-500 shadow-md',
        isDragging && 'shadow-lg opacity-90',
      )}
      style={{
        left: `${pos.x}px`,
        top: `${pos.y}px`,
        zIndex: isSelected ? 20 : isDragging ? 30 : 10,
      }}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      onMouseDown={handleMouseDown}
    >
      {/* Entry indicator */}
      {isEntryNode && (
        <div
          className="absolute -top-2 -left-2 w-4 h-4 rounded-full bg-green-500 border-2 border-white dark:border-gray-900"
          title={t('graphWorkflow.nodeTypes.entryNode')}
        />
      )}

      {/* Step type badge */}
      <div className="text-[10px] uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-1">
        {STEP_TYPE_LABEL_KEYS[stepType] ? t(STEP_TYPE_LABEL_KEYS[stepType]) : stepType}
      </div>

      {/* Node name */}
      <div className="text-sm font-medium text-gray-900 dark:text-white truncate max-w-[180px]">{stepName}</div>
    </div>
  );
}
