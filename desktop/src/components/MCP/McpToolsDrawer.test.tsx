import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { McpToolsDrawer } from './McpToolsDrawer';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'mcp.toolSchemaSummary') {
        return `${String(options?.parameters)} params / ${String(options?.required)} required`;
      }
      const map: Record<string, string> = {
        'mcp.toolsDrawerTitle': 'MCP Tools',
        'mcp.toolsSearchPlaceholder': 'Search tools',
        'mcp.copyQualifiedName': 'Copy name',
        'mcp.parallelSafe': 'Parallel-safe',
        'mcp.parallelUnsafe': 'Sequential recommended',
        'mcp.status.unknown': 'Unknown',
        'mcp.connectedAtMeta': 'connected_at={{value}}',
        'mcp.connectionMeta': 'protocol={{protocol}} tools={{count}}',
      };
      return map[key] || key;
    },
  }),
}));

describe('McpToolsDrawer', () => {
  it('renders detailed metadata for each tool', () => {
    const onQueryChange = vi.fn();

    render(
      <McpToolsDrawer
        open={true}
        server={{
          server_id: 's1',
          server_name: 'Server One',
          connection_state: 'connected',
          tool_names: ['list_files'],
          qualified_tool_names: ['mcp:s1:list_files'],
          protocol_version: '2025-03-26',
          connected_at: null,
          last_error: null,
          retry_count: 0,
        }}
        tools={[
          {
            qualified_name: 'mcp:s1:list_files',
            tool_name: 'list_files',
            description: 'List files in current directory',
            input_schema: {
              type: 'object',
              properties: {
                path: { type: 'string' },
                recursive: { type: 'boolean' },
              },
              required: ['path'],
            },
            is_parallel_safe: true,
          },
        ]}
        query=""
        onQueryChange={onQueryChange}
        loading={false}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText('list_files')).toBeInTheDocument();
    expect(screen.getByText('mcp:s1:list_files')).toBeInTheDocument();
    expect(screen.getByText('List files in current directory')).toBeInTheDocument();
    expect(screen.getByText('2 params / 1 required')).toBeInTheDocument();
    expect(screen.getByText('Parallel-safe')).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText('Search tools'), { target: { value: 'files' } });
    expect(onQueryChange).toHaveBeenCalledWith('files');
  });
});
