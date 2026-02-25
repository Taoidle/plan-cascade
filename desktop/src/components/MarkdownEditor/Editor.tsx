/**
 * Editor Component
 *
 * Monaco Editor integration for markdown editing with syntax highlighting.
 * Supports dark/light themes, keyboard shortcuts, and formatting toolbar.
 */

import { useRef, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import MonacoEditor, { OnMount, OnChange } from '@monaco-editor/react';
import type { editor } from 'monaco-editor';
import {
  FontBoldIcon,
  FontItalicIcon,
  Link1Icon,
  CodeIcon,
  QuoteIcon,
  ListBulletIcon,
  CheckboxIcon,
} from '@radix-ui/react-icons';

interface EditorProps {
  content: string;
  onChange: (content: string) => void;
  onSave: () => void;
  isDark?: boolean;
  readOnly?: boolean;
  fileName?: string;
}

/** Toolbar button component */
interface ToolbarButtonProps {
  icon: React.ReactNode;
  title: string;
  onClick: () => void;
  disabled?: boolean;
}

function ToolbarButton({ icon, title, onClick, disabled }: ToolbarButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={clsx(
        'p-1.5 rounded',
        'text-gray-600 dark:text-gray-400',
        'hover:bg-gray-200 dark:hover:bg-gray-700',
        'hover:text-gray-900 dark:hover:text-white',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        'transition-colors',
      )}
    >
      {icon}
    </button>
  );
}

export function Editor({ content, onChange, onSave, isDark = false, readOnly = false, fileName }: EditorProps) {
  const { t } = useTranslation();
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);

  // Handle editor mount
  const handleEditorMount: OnMount = useCallback(
    (editor, monaco) => {
      editorRef.current = editor;

      // Add Ctrl+S keyboard shortcut for save
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
        onSave();
      });

      // Add Ctrl+B for bold
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyB, () => {
        insertFormatting('**', '**');
      });

      // Add Ctrl+I for italic
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyI, () => {
        insertFormatting('*', '*');
      });

      // Focus the editor
      editor.focus();
    },
    [onSave],
  );

  // Handle content change
  const handleChange: OnChange = useCallback(
    (value) => {
      if (value !== undefined) {
        onChange(value);
      }
    },
    [onChange],
  );

  // Insert formatting around selection
  const insertFormatting = useCallback((before: string, after: string) => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    const model = editor.getModel();
    if (!model) return;

    const selectedText = model.getValueInRange(selection);

    editor.executeEdits('toolbar', [
      {
        range: selection,
        text: `${before}${selectedText}${after}`,
        forceMoveMarkers: true,
      },
    ]);

    // If no text was selected, place cursor between the markers
    if (selectedText === '') {
      const newPosition = {
        lineNumber: selection.startLineNumber,
        column: selection.startColumn + before.length,
      };
      editor.setPosition(newPosition);
    }

    editor.focus();
  }, []);

  // Insert heading at line start
  const insertHeading = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    const model = editor.getModel();
    if (!model) return;

    const lineNumber = selection.startLineNumber;
    const lineContent = model.getLineContent(lineNumber);

    // Check if line already starts with a heading
    const headingMatch = lineContent.match(/^(#{1,6})\s/);

    if (headingMatch) {
      // Cycle through heading levels or remove
      const currentLevel = headingMatch[1].length;
      if (currentLevel >= 6) {
        // Remove heading
        editor.executeEdits('toolbar', [
          {
            range: {
              startLineNumber: lineNumber,
              startColumn: 1,
              endLineNumber: lineNumber,
              endColumn: headingMatch[0].length + 1,
            },
            text: '',
            forceMoveMarkers: true,
          },
        ]);
      } else {
        // Increase heading level
        editor.executeEdits('toolbar', [
          {
            range: {
              startLineNumber: lineNumber,
              startColumn: 1,
              endLineNumber: lineNumber,
              endColumn: 1,
            },
            text: '#',
            forceMoveMarkers: true,
          },
        ]);
      }
    } else {
      // Add heading
      editor.executeEdits('toolbar', [
        {
          range: {
            startLineNumber: lineNumber,
            startColumn: 1,
            endLineNumber: lineNumber,
            endColumn: 1,
          },
          text: '## ',
          forceMoveMarkers: true,
        },
      ]);
    }

    editor.focus();
  }, []);

  // Insert link
  const insertLink = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    const model = editor.getModel();
    if (!model) return;

    const selectedText = model.getValueInRange(selection);

    if (selectedText) {
      // Wrap selection in link syntax
      editor.executeEdits('toolbar', [
        {
          range: selection,
          text: `[${selectedText}](url)`,
          forceMoveMarkers: true,
        },
      ]);
    } else {
      // Insert link placeholder
      editor.executeEdits('toolbar', [
        {
          range: selection,
          text: '[link text](url)',
          forceMoveMarkers: true,
        },
      ]);
    }

    editor.focus();
  }, []);

  // Insert code block
  const insertCodeBlock = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    const model = editor.getModel();
    if (!model) return;

    const selectedText = model.getValueInRange(selection);

    if (selectedText) {
      // Wrap in code block
      editor.executeEdits('toolbar', [
        {
          range: selection,
          text: `\`\`\`\n${selectedText}\n\`\`\``,
          forceMoveMarkers: true,
        },
      ]);
    } else {
      // Insert code block placeholder
      editor.executeEdits('toolbar', [
        {
          range: selection,
          text: '```\ncode here\n```',
          forceMoveMarkers: true,
        },
      ]);
    }

    editor.focus();
  }, []);

  // Insert list item
  const insertList = useCallback((ordered: boolean = false) => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    const prefix = ordered ? '1. ' : '- ';

    editor.executeEdits('toolbar', [
      {
        range: {
          startLineNumber: selection.startLineNumber,
          startColumn: 1,
          endLineNumber: selection.startLineNumber,
          endColumn: 1,
        },
        text: prefix,
        forceMoveMarkers: true,
      },
    ]);

    editor.focus();
  }, []);

  // Insert task list
  const insertTaskList = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    editor.executeEdits('toolbar', [
      {
        range: {
          startLineNumber: selection.startLineNumber,
          startColumn: 1,
          endLineNumber: selection.startLineNumber,
          endColumn: 1,
        },
        text: '- [ ] ',
        forceMoveMarkers: true,
      },
    ]);

    editor.focus();
  }, []);

  // Insert quote
  const insertQuote = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;

    const selection = editor.getSelection();
    if (!selection) return;

    editor.executeEdits('toolbar', [
      {
        range: {
          startLineNumber: selection.startLineNumber,
          startColumn: 1,
          endLineNumber: selection.startLineNumber,
          endColumn: 1,
        },
        text: '> ',
        forceMoveMarkers: true,
      },
    ]);

    editor.focus();
  }, []);

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">
        <ToolbarButton
          icon={<FontBoldIcon className="w-4 h-4" />}
          title={`${t('markdownEditor.editor.toolbar.bold')} (Ctrl+B)`}
          onClick={() => insertFormatting('**', '**')}
          disabled={readOnly}
        />
        <ToolbarButton
          icon={<FontItalicIcon className="w-4 h-4" />}
          title={`${t('markdownEditor.editor.toolbar.italic')} (Ctrl+I)`}
          onClick={() => insertFormatting('*', '*')}
          disabled={readOnly}
        />
        <div className="w-px h-4 bg-gray-300 dark:bg-gray-600 mx-1" />
        <ToolbarButton
          icon={<span className="text-xs font-bold">H</span>}
          title={t('markdownEditor.editor.toolbar.heading')}
          onClick={insertHeading}
          disabled={readOnly}
        />
        <ToolbarButton
          icon={<Link1Icon className="w-4 h-4" />}
          title={t('markdownEditor.editor.toolbar.link')}
          onClick={insertLink}
          disabled={readOnly}
        />
        <div className="w-px h-4 bg-gray-300 dark:bg-gray-600 mx-1" />
        <ToolbarButton
          icon={<CodeIcon className="w-4 h-4" />}
          title={t('markdownEditor.editor.toolbar.code')}
          onClick={() => insertFormatting('`', '`')}
          disabled={readOnly}
        />
        <ToolbarButton
          icon={<span className="text-xs font-mono">{'{}'}</span>}
          title={t('markdownEditor.editor.toolbar.codeBlock')}
          onClick={insertCodeBlock}
          disabled={readOnly}
        />
        <ToolbarButton
          icon={<QuoteIcon className="w-4 h-4" />}
          title={t('markdownEditor.editor.toolbar.quote')}
          onClick={insertQuote}
          disabled={readOnly}
        />
        <div className="w-px h-4 bg-gray-300 dark:bg-gray-600 mx-1" />
        <ToolbarButton
          icon={<ListBulletIcon className="w-4 h-4" />}
          title={t('markdownEditor.editor.toolbar.list')}
          onClick={() => insertList(false)}
          disabled={readOnly}
        />
        <ToolbarButton
          icon={<span className="text-xs font-bold">1.</span>}
          title={t('markdownEditor.editor.toolbar.orderedList')}
          onClick={() => insertList(true)}
          disabled={readOnly}
        />
        <ToolbarButton
          icon={<CheckboxIcon className="w-4 h-4" />}
          title={t('markdownEditor.editor.toolbar.taskList')}
          onClick={insertTaskList}
          disabled={readOnly}
        />

        {/* Spacer */}
        <div className="flex-1" />

        {/* File name indicator */}
        {fileName && (
          <span className="text-xs text-gray-500 dark:text-gray-400 truncate max-w-[200px]">{fileName}</span>
        )}
      </div>

      {/* Editor */}
      <div className="flex-1 min-h-0">
        <MonacoEditor
          height="100%"
          language="markdown"
          theme={isDark ? 'vs-dark' : 'light'}
          value={content}
          onChange={handleChange}
          onMount={handleEditorMount}
          options={{
            readOnly,
            minimap: { enabled: false },
            wordWrap: 'on',
            lineNumbers: 'on',
            fontSize: 14,
            fontFamily: 'JetBrains Mono, Consolas, monospace',
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 2,
            insertSpaces: true,
            renderWhitespace: 'selection',
            bracketPairColorization: { enabled: true },
            padding: { top: 8, bottom: 8 },
            cursorBlinking: 'smooth',
            cursorSmoothCaretAnimation: 'on',
            smoothScrolling: true,
          }}
        />
      </div>
    </div>
  );
}

export default Editor;
