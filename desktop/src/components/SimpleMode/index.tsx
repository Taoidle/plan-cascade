/**
 * SimpleMode Component
 *
 * Container for Simple mode interface.
 * Provides a streamlined experience with:
 * - Single input for task description
 * - Automatic strategy selection
 * - Progress visualization
 * - Results display
 */

import { useState } from 'react';
import { InputBox } from './InputBox';
import { ProgressView } from './ProgressView';
import { ResultView } from './ResultView';
import { useExecutionStore } from '../../store/execution';

export function SimpleMode() {
  const { status, start, result } = useExecutionStore();
  const [description, setDescription] = useState('');

  const handleStart = async () => {
    if (!description.trim()) return;
    await start(description, 'simple');
  };

  const isRunning = status === 'running';
  const isCompleted = status === 'completed' || status === 'failed';

  return (
    <div className="h-full flex flex-col p-6">
      {/* Input Area */}
      <div className="max-w-2xl mx-auto w-full">
        <InputBox
          value={description}
          onChange={setDescription}
          onSubmit={handleStart}
          disabled={isRunning}
          placeholder="Describe what you want to build..."
        />
      </div>

      {/* Progress/Results Area */}
      <div className="flex-1 mt-8 overflow-auto">
        {isRunning && <ProgressView />}
        {isCompleted && <ResultView result={result} />}

        {/* Empty state when idle */}
        {status === 'idle' && (
          <div className="flex flex-col items-center justify-center h-full text-center">
            <div className="text-6xl mb-4">
              <span role="img" aria-label="rocket">&#128640;</span>
            </div>
            <h2 className="text-xl font-semibold text-gray-700 dark:text-gray-300 mb-2">
              Ready to build
            </h2>
            <p className="text-gray-500 dark:text-gray-400 max-w-md">
              Describe your task above and Plan Cascade will automatically
              break it down, create a plan, and execute it step by step.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

export default SimpleMode;
