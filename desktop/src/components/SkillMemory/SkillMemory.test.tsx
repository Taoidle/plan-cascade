/**
 * SkillMemory Component Tests
 *
 * Tests for SkillSourceBadge, CategoryBadge, ImportanceBar, EmptyState,
 * SkillRow, MemoryRow, ActiveSkillsIndicator, and SkillMemoryToast.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import type { SkillSummary, MemoryEntry } from '../../types/skillMemory';

// Mock react-i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      const translations: Record<string, string> = {
        'skillPanel.title': 'Skills & Memory',
        'skillPanel.manageAll': 'Manage All...',
        'skillPanel.loading': 'Loading...',
        'skillPanel.detectedSkills': 'Auto-Detected Skills',
        'skillPanel.projectSkills': 'Project Skills',
        'skillPanel.memories': 'Memories',
        'skillPanel.noDetectedSkills': 'No skills detected for this project',
        'skillPanel.noProjectSkills': 'No project skills configured',
        'skillPanel.noMemories': 'No memories stored yet',
        'skillPanel.dialogTitle': 'Skills & Memory Management',
        'skillPanel.skillsTab': 'Skills',
        'skillPanel.memoryTab': 'Memory',
        'skillPanel.close': 'Close',
        'skillPanel.activeSkillsTooltip': `${opts?.count ?? 0} active skill(s)`,
      };
      return translations[key] || key;
    },
    i18n: { language: 'en' },
  }),
}));

// Mock settings store
vi.mock('../../store/settings', () => ({
  useSettingsStore: vi.fn((selector) => {
    const state = {
      workspacePath: '/test/project',
    };
    return typeof selector === 'function' ? selector(state) : state;
  }),
}));

// Mock skillMemory store - we'll override per test
const mockStore = {
  skills: [] as SkillSummary[],
  skillsLoading: false,
  memories: [] as MemoryEntry[],
  memoriesLoading: false,
  panelOpen: false,
  dialogOpen: false,
  activeTab: 'skills' as const,
  toastMessage: null as string | null,
  toastType: 'info' as 'info' | 'success' | 'error',
  loadSkills: vi.fn(),
  loadMemories: vi.fn(),
  loadMemoryStats: vi.fn(),
  toggleSkill: vi.fn(),
  togglePanel: vi.fn(),
  openDialog: vi.fn(),
  closeDialog: vi.fn(),
  setActiveTab: vi.fn(),
  clearToast: vi.fn(),
};

vi.mock('../../store/skillMemory', () => ({
  useSkillMemoryStore: vi.fn((selector) => {
    return typeof selector === 'function' ? selector(mockStore) : mockStore;
  }),
}));

import { SkillSourceBadge } from './SkillSourceBadge';
import { CategoryBadge } from './CategoryBadge';
import { ImportanceBar } from './ImportanceBar';
import { EmptyState } from './EmptyState';
import { ActiveSkillsIndicator } from './ActiveSkillsIndicator';
import { SkillMemoryToast } from './SkillMemoryToast';
import { SkillRow } from '../SimpleMode/SkillRow';
import { MemoryRow } from '../SimpleMode/MemoryRow';

// ============================================================================
// Helpers
// ============================================================================

function createMockSkill(overrides: Partial<SkillSummary> = {}): SkillSummary {
  return {
    id: 'skill-1',
    name: 'Test Skill',
    description: 'A test skill',
    version: null,
    tags: ['test'],
    source: { type: 'builtin' },
    priority: 10,
    enabled: true,
    detected: false,
    user_invocable: false,
    has_hooks: false,
    inject_into: ['always'],
    path: '/skills/test.md',
    ...overrides,
  };
}

function createMockMemory(overrides: Partial<MemoryEntry> = {}): MemoryEntry {
  return {
    id: 'mem-1',
    project_path: '/test/project',
    category: 'fact',
    content: 'Test memory content',
    keywords: ['test'],
    importance: 0.5,
    access_count: 0,
    source_session_id: null,
    source_context: null,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    last_accessed_at: '2025-01-01T00:00:00Z',
    ...overrides,
  };
}

// ============================================================================
// SkillSourceBadge Tests
// ============================================================================

describe('SkillSourceBadge', () => {
  it('should render builtin badge with gray styling', () => {
    render(<SkillSourceBadge source={{ type: 'builtin' }} />);
    const badge = screen.getByTestId('skill-source-badge');
    expect(badge).toHaveTextContent('Built-in');
    expect(badge.className).toContain('bg-gray-100');
  });

  it('should render external badge with blue styling', () => {
    render(<SkillSourceBadge source={{ type: 'external', source_name: 'React' }} />);
    const badge = screen.getByTestId('skill-source-badge');
    expect(badge).toHaveTextContent('React');
    expect(badge.className).toContain('bg-blue-100');
  });

  it('should render project_local badge with green styling', () => {
    render(<SkillSourceBadge source={{ type: 'project_local' }} />);
    const badge = screen.getByTestId('skill-source-badge');
    expect(badge).toHaveTextContent('Project');
    expect(badge.className).toContain('bg-green-100');
  });

  it('should render generated badge with purple styling', () => {
    render(<SkillSourceBadge source={{ type: 'generated' }} />);
    const badge = screen.getByTestId('skill-source-badge');
    expect(badge).toHaveTextContent('Generated');
    expect(badge.className).toContain('bg-purple-100');
  });

  it('should render compact mode with smaller text', () => {
    render(<SkillSourceBadge source={{ type: 'builtin' }} compact />);
    const badge = screen.getByTestId('skill-source-badge');
    expect(badge.className).toContain('text-2xs');
  });
});

// ============================================================================
// CategoryBadge Tests
// ============================================================================

describe('CategoryBadge', () => {
  it('should render preference badge with blue styling', () => {
    render(<CategoryBadge category="preference" />);
    const badge = screen.getByTestId('category-badge');
    expect(badge).toHaveTextContent('Preference');
    expect(badge.className).toContain('bg-blue-100');
  });

  it('should render convention badge with amber styling', () => {
    render(<CategoryBadge category="convention" />);
    const badge = screen.getByTestId('category-badge');
    expect(badge).toHaveTextContent('Convention');
    expect(badge.className).toContain('bg-amber-100');
  });

  it('should render correction badge with red styling', () => {
    render(<CategoryBadge category="correction" />);
    const badge = screen.getByTestId('category-badge');
    expect(badge).toHaveTextContent('Correction');
    expect(badge.className).toContain('bg-red-100');
  });

  it('should render pattern badge with green styling', () => {
    render(<CategoryBadge category="pattern" />);
    const badge = screen.getByTestId('category-badge');
    expect(badge).toHaveTextContent('Pattern');
    expect(badge.className).toContain('bg-green-100');
  });

  it('should render fact badge with purple styling', () => {
    render(<CategoryBadge category="fact" />);
    const badge = screen.getByTestId('category-badge');
    expect(badge).toHaveTextContent('Fact');
    expect(badge.className).toContain('bg-purple-100');
  });
});

// ============================================================================
// ImportanceBar Tests
// ============================================================================

describe('ImportanceBar', () => {
  it('should render with correct width percentage', () => {
    render(<ImportanceBar value={0.7} />);
    const bar = screen.getByTestId('importance-bar');
    expect(bar).toBeInTheDocument();
    const inner = bar.querySelector('[role="progressbar"]');
    expect(inner).toHaveAttribute('aria-valuenow', '70');
  });

  it('should clamp values to 0-1 range', () => {
    render(<ImportanceBar value={1.5} />);
    const inner = screen.getByRole('progressbar');
    expect(inner).toHaveAttribute('aria-valuenow', '100');
  });

  it('should show label when showLabel is true', () => {
    render(<ImportanceBar value={0.75} showLabel />);
    expect(screen.getByText('75%')).toBeInTheDocument();
  });

  it('should not show label by default', () => {
    render(<ImportanceBar value={0.5} />);
    expect(screen.queryByText('50%')).not.toBeInTheDocument();
  });

  it('should use amber color for medium importance', () => {
    render(<ImportanceBar value={0.65} />);
    const inner = screen.getByRole('progressbar');
    expect(inner.className).toContain('bg-amber-500');
  });

  it('should use red color for high importance', () => {
    render(<ImportanceBar value={0.9} />);
    const inner = screen.getByRole('progressbar');
    expect(inner.className).toContain('bg-red-500');
  });
});

// ============================================================================
// EmptyState Tests
// ============================================================================

describe('EmptyState', () => {
  it('should render title and description', () => {
    render(<EmptyState title="No items" description="Nothing to show" />);
    expect(screen.getByText('No items')).toBeInTheDocument();
    expect(screen.getByText('Nothing to show')).toBeInTheDocument();
  });

  it('should render action button when provided', () => {
    const onClick = vi.fn();
    render(<EmptyState title="Empty" action={{ label: 'Add Item', onClick }} />);
    const button = screen.getByText('Add Item');
    expect(button).toBeInTheDocument();
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalled();
  });

  it('should render default icon when no custom icon provided', () => {
    render(<EmptyState title="Empty" />);
    const container = screen.getByTestId('empty-state');
    expect(container.querySelector('svg')).toBeInTheDocument();
  });
});

// ============================================================================
// SkillRow Tests
// ============================================================================

describe('SkillRow', () => {
  it('should render skill name and source badge', () => {
    const skill = createMockSkill({ name: 'React Best Practices' });
    render(<SkillRow skill={skill} onToggle={vi.fn()} />);
    expect(screen.getByText('React Best Practices')).toBeInTheDocument();
    expect(screen.getByTestId('skill-source-badge')).toBeInTheDocument();
  });

  it('should render checkbox reflecting enabled state', () => {
    const skill = createMockSkill({ enabled: true });
    render(<SkillRow skill={skill} onToggle={vi.fn()} />);
    const checkbox = screen.getByRole('checkbox');
    expect(checkbox).toBeChecked();
  });

  it('should call onToggle when checkbox is clicked', () => {
    const onToggle = vi.fn();
    const skill = createMockSkill({ id: 'skill-x', enabled: true });
    render(<SkillRow skill={skill} onToggle={onToggle} />);
    fireEvent.click(screen.getByRole('checkbox'));
    expect(onToggle).toHaveBeenCalledWith('skill-x', false);
  });

  it('should call onClick when row is clicked', () => {
    const onClick = vi.fn();
    const skill = createMockSkill();
    render(<SkillRow skill={skill} onToggle={vi.fn()} onClick={onClick} />);
    fireEvent.click(screen.getByTestId('skill-row-skill-1'));
    expect(onClick).toHaveBeenCalledWith(skill);
  });
});

// ============================================================================
// MemoryRow Tests
// ============================================================================

describe('MemoryRow', () => {
  it('should render category badge and truncated content', () => {
    const memory = createMockMemory({ content: 'This is a test memory content' });
    render(<MemoryRow memory={memory} />);
    expect(screen.getByTestId('category-badge')).toBeInTheDocument();
    expect(screen.getByText('This is a test memory content')).toBeInTheDocument();
  });

  it('should truncate long content', () => {
    const longContent = 'A'.repeat(100);
    const memory = createMockMemory({ content: longContent });
    render(<MemoryRow memory={memory} />);
    expect(screen.getByText(longContent.slice(0, 80) + '...')).toBeInTheDocument();
  });

  it('should call onClick when clicked', () => {
    const onClick = vi.fn();
    const memory = createMockMemory();
    render(<MemoryRow memory={memory} onClick={onClick} />);
    fireEvent.click(screen.getByTestId('memory-row-mem-1'));
    expect(onClick).toHaveBeenCalledWith(memory);
  });

  it('should render importance bar', () => {
    const memory = createMockMemory({ importance: 0.8 });
    render(<MemoryRow memory={memory} />);
    expect(screen.getByTestId('importance-bar')).toBeInTheDocument();
  });
});

// ============================================================================
// ActiveSkillsIndicator Tests
// ============================================================================

describe('ActiveSkillsIndicator', () => {
  beforeEach(() => {
    mockStore.skills = [];
  });

  it('should not render when no skills are enabled', () => {
    mockStore.skills = [];
    const { container } = render(<ActiveSkillsIndicator />);
    expect(container.firstChild).toBeNull();
  });

  it('should render count of enabled skills', () => {
    mockStore.skills = [
      createMockSkill({ id: 'a', enabled: true }),
      createMockSkill({ id: 'b', enabled: true }),
      createMockSkill({ id: 'c', enabled: false }),
    ] as SkillSummary[];
    render(<ActiveSkillsIndicator />);
    expect(screen.getByTestId('active-skills-indicator')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('should open dialog when clicked', () => {
    mockStore.skills = [createMockSkill({ enabled: true })] as SkillSummary[];
    render(<ActiveSkillsIndicator />);
    fireEvent.click(screen.getByTestId('active-skills-indicator'));
    expect(mockStore.openDialog).toHaveBeenCalledWith('skills');
  });
});

// ============================================================================
// SkillMemoryToast Tests
// ============================================================================

describe('SkillMemoryToast', () => {
  it('should not render when no toast message', () => {
    mockStore.toastMessage = null;
    const { container } = render(<SkillMemoryToast />);
    expect(container.querySelector('[data-testid="skill-memory-toast"]')).toBeNull();
  });

  it('should render toast with message', () => {
    mockStore.toastMessage = 'Skill enabled';
    mockStore.toastType = 'success';
    render(<SkillMemoryToast />);
    expect(screen.getByTestId('skill-memory-toast')).toBeInTheDocument();
    expect(screen.getByText('Skill enabled')).toBeInTheDocument();
  });

  it('should call clearToast when close button is clicked', () => {
    mockStore.toastMessage = 'Test toast';
    mockStore.toastType = 'info';
    render(<SkillMemoryToast />);
    // The close button is inside the toast
    const buttons = screen.getByTestId('skill-memory-toast').querySelectorAll('button');
    fireEvent.click(buttons[0]);
    expect(mockStore.clearToast).toHaveBeenCalled();
  });
});
