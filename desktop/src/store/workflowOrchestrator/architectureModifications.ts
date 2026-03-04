import type { TaskPrd } from '../taskMode';
import type { ArchitectureReviewCardData } from '../../types/workflowCard';

function uniqueDeps(dependencies: string[]): string[] {
  return [...new Set(dependencies.filter((dependency) => dependency.trim().length > 0))];
}

function nextGeneratedStoryId(stories: TaskPrd['stories']): string {
  const used = new Set(stories.map((story) => story.id));
  let index = stories.length + 1;
  while (used.has(`S${String(index).padStart(3, '0')}`)) index += 1;
  return `S${String(index).padStart(3, '0')}`;
}

function recomputeBatches(stories: TaskPrd['stories'], maxParallel: number): TaskPrd['batches'] {
  const remaining = new Set(stories.map((story) => story.id));
  const inDegree = new Map<string, number>();
  const dependents = new Map<string, string[]>();
  const validIds = new Set(stories.map((story) => story.id));

  for (const story of stories) {
    const deps = story.dependencies.filter((dependency) => validIds.has(dependency));
    inDegree.set(story.id, deps.length);
    for (const dependency of deps) {
      dependents.set(dependency, [...(dependents.get(dependency) ?? []), story.id]);
    }
  }

  const batches: TaskPrd['batches'] = [];
  while (remaining.size > 0) {
    const ready = [...remaining].filter((id) => (inDegree.get(id) ?? 0) === 0).sort();
    if (ready.length === 0) {
      throw new Error('Dependency cycle detected while applying architecture modifications');
    }
    const chunkSize = Math.max(1, maxParallel);
    for (let index = 0; index < ready.length; index += chunkSize) {
      const chunk = ready.slice(index, index + chunkSize);
      batches.push({ index: batches.length, storyIds: chunk });
    }
    for (const id of ready) {
      remaining.delete(id);
      const children = dependents.get(id) ?? [];
      for (const child of children) {
        inDegree.set(child, Math.max(0, (inDegree.get(child) ?? 0) - 1));
      }
    }
  }
  return batches;
}

export function applyArchitectureModifications(
  prd: TaskPrd,
  modifications: ArchitectureReviewCardData['prdModifications'],
  maxParallel: number,
): TaskPrd {
  const nextPrd: TaskPrd = {
    ...prd,
    stories: prd.stories.map((story) => ({
      ...story,
      dependencies: [...story.dependencies],
      acceptanceCriteria: [...story.acceptanceCriteria],
    })),
    batches: [...prd.batches],
  };

  const storyMap = new Map(nextPrd.stories.map((story) => [story.id, story]));

  for (const modification of modifications) {
    const targetId = modification.targetStoryId || null;
    switch (modification.type) {
      case 'update_story': {
        if (!targetId) break;
        const story = storyMap.get(targetId);
        if (!story) break;
        const payload = modification.payload || {};
        story.title = payload.title ?? story.title;
        story.description = payload.description ?? story.description;
        story.priority = payload.priority ?? story.priority;
        if (payload.dependencies) story.dependencies = uniqueDeps(payload.dependencies);
        if (payload.acceptanceCriteria) story.acceptanceCriteria = [...payload.acceptanceCriteria];
        break;
      }
      case 'add_story': {
        const payloadStory = modification.payload?.story;
        const nextId = payloadStory?.id?.trim() || nextGeneratedStoryId(nextPrd.stories);
        const normalizedId = storyMap.has(nextId) ? nextGeneratedStoryId(nextPrd.stories) : nextId;
        const newStory = {
          id: normalizedId,
          title: payloadStory?.title ?? modification.payload?.title ?? 'New Story',
          description: payloadStory?.description ?? modification.payload?.description ?? modification.reason,
          priority: payloadStory?.priority ?? modification.payload?.priority ?? 'medium',
          dependencies: uniqueDeps(payloadStory?.dependencies ?? modification.payload?.dependencies ?? []),
          acceptanceCriteria: [...(payloadStory?.acceptanceCriteria ?? modification.payload?.acceptanceCriteria ?? [])],
        };
        nextPrd.stories.push(newStory);
        storyMap.set(newStory.id, newStory);
        break;
      }
      case 'remove_story': {
        if (!targetId) break;
        nextPrd.stories = nextPrd.stories.filter((story) => story.id !== targetId);
        for (const story of nextPrd.stories) {
          story.dependencies = story.dependencies.filter((dependency) => dependency !== targetId);
        }
        storyMap.delete(targetId);
        break;
      }
      case 'split_story': {
        if (!targetId) break;
        const target = storyMap.get(targetId);
        const splitStories = modification.payload?.stories ?? [];
        if (!target || splitStories.length < 2) break;
        const index = nextPrd.stories.findIndex((story) => story.id === targetId);
        if (index < 0) break;
        const createdIds: string[] = [];
        const created = splitStories.map((item, splitIndex) => {
          const candidate = item.id?.trim() || `${targetId}-${splitIndex + 1}`;
          const id = storyMap.has(candidate) ? `${targetId}-${splitIndex + 1}-${Date.now()}` : candidate;
          createdIds.push(id);
          const dependencies =
            item.dependencies.length > 0
              ? item.dependencies
              : splitIndex === 0
                ? target.dependencies
                : [createdIds[splitIndex - 1]];
          const story = {
            id,
            title: item.title,
            description: item.description,
            priority: item.priority || target.priority,
            dependencies: uniqueDeps(dependencies),
            acceptanceCriteria: [...(item.acceptanceCriteria ?? [])],
          };
          storyMap.set(id, story);
          return story;
        });
        nextPrd.stories.splice(index, 1, ...created);
        storyMap.delete(targetId);
        for (const story of nextPrd.stories) {
          if (story.id === targetId) continue;
          if (story.dependencies.includes(targetId)) {
            const remapped = modification.payload?.dependencyRemap?.[story.id] ?? [createdIds[createdIds.length - 1]];
            story.dependencies = uniqueDeps(
              story.dependencies.flatMap((dependency) => (dependency === targetId ? remapped : [dependency])),
            );
          }
        }
        break;
      }
      case 'merge_story': {
        if (!targetId) break;
        const story = storyMap.get(targetId);
        if (!story) break;
        const payload = modification.payload || {};
        story.title = payload.title ?? story.title;
        story.description = payload.description ?? story.description;
        story.priority = payload.priority ?? story.priority;
        if (payload.dependencies) story.dependencies = uniqueDeps(payload.dependencies);
        if (payload.acceptanceCriteria) story.acceptanceCriteria = [...payload.acceptanceCriteria];
        break;
      }
    }
  }

  nextPrd.batches = recomputeBatches(nextPrd.stories, maxParallel);
  return nextPrd;
}
