/**
 * AgentPipelineRunner Component
 *
 * Displays real-time execution events from a running agent pipeline.
 */

import { clsx } from 'clsx';
import { useAgentComposerStore } from '../../store/agentComposer';
import type { AgentEvent } from '../../types/agentComposer';

export function AgentPipelineRunner() {
  const { executionEvents, isExecuting, clearExecutionEvents } =
    useAgentComposerStore();

  if (executionEvents.length === 0 && !isExecuting) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-gray-500 dark:text-gray-400">
        No execution in progress. Run a pipeline to see events here.
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">
          Execution Events
          {isExecuting && (
            <span className="ml-2 inline-block w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          )}
        </h3>
        {executionEvents.length > 0 && (
          <button
            onClick={clearExecutionEvents}
            className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          >
            Clear
          </button>
        )}
      </div>

      <div
        className={clsx(
          'flex-1 overflow-auto rounded-lg p-3',
          'bg-gray-900 text-gray-100 font-mono text-xs',
          'space-y-1'
        )}
      >
        {executionEvents.map((event, i) => (
          <EventLine key={i} event={event} />
        ))}
      </div>
    </div>
  );
}

function EventLine({ event }: { event: AgentEvent }) {
  switch (event.type) {
    case 'text_delta':
      return (
        <span className="text-green-400">{event.content}</span>
      );
    case 'thinking_delta':
      return (
        <span className="text-blue-400 italic">{event.content}</span>
      );
    case 'tool_call':
      return (
        <div className="text-yellow-400">
          [Tool Call] {event.name}({event.args.length > 80 ? event.args.slice(0, 80) + '...' : event.args})
        </div>
      );
    case 'tool_result':
      return (
        <div className="text-cyan-400">
          [Tool Result] {event.result.length > 120 ? event.result.slice(0, 120) + '...' : event.result}
        </div>
      );
    case 'state_update':
      return (
        <div className="text-purple-400">
          [State] {event.key} = {JSON.stringify(event.value)}
        </div>
      );
    case 'agent_transfer':
      return (
        <div className="text-orange-400">
          [Transfer] -&gt; {event.target}: {event.message}
        </div>
      );
    case 'done':
      return (
        <div className="text-green-300 font-bold mt-1">
          [Done] {event.output ?? 'Completed'}
        </div>
      );
    default:
      return null;
  }
}

export default AgentPipelineRunner;
