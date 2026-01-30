/**
 * Projects Component
 *
 * Main Projects page with two-panel layout:
 * - Left: Project browser with search and sorting
 * - Right: Session list for selected project
 */

import { clsx } from 'clsx';
import { useProjectsStore } from '../../store/projects';
import { ProjectBrowser } from './ProjectBrowser';
import { SessionList } from './SessionList';

export function Projects() {
  const { selectedProject } = useProjectsStore();

  return (
    <div className="h-full flex">
      {/* Left Panel - Projects */}
      <div
        className={clsx(
          'h-full border-r border-gray-200 dark:border-gray-700',
          'bg-gray-50 dark:bg-gray-900',
          // Responsive: full width on mobile when no project selected, 1/3 on desktop
          selectedProject ? 'hidden md:block md:w-1/3 lg:w-1/4' : 'w-full md:w-1/3 lg:w-1/4'
        )}
      >
        <ProjectBrowser />
      </div>

      {/* Right Panel - Sessions */}
      <div
        className={clsx(
          'h-full flex-1',
          'bg-white dark:bg-gray-950',
          // Hide on mobile when no project selected
          selectedProject ? 'w-full md:w-2/3 lg:w-3/4' : 'hidden md:block'
        )}
      >
        <SessionList />
      </div>
    </div>
  );
}

export { ProjectBrowser } from './ProjectBrowser';
export { ProjectCard } from './ProjectCard';
export { SessionList } from './SessionList';
export { SessionCard } from './SessionCard';
export { SessionDetails } from './SessionDetails';

export default Projects;
