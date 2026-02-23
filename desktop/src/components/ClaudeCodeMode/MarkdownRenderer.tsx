/**
 * MarkdownRenderer Component
 *
 * Enhanced markdown rendering with GFM support, syntax highlighting,
 * and math expressions rendering using KaTeX.
 *
 * Story 011-1: Enhanced Markdown Rendering with Syntax Highlighting
 */

import { memo, useMemo } from 'react';
import ReactMarkdown, { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import rehypeRaw from 'rehype-raw';
import { clsx } from 'clsx';
import { CodeBlock } from './CodeBlock';

// Import KaTeX CSS (needs to be imported in main.tsx or index.css)
// import 'katex/dist/katex.min.css';

// ============================================================================
// Types
// ============================================================================

interface MarkdownRendererProps {
  content: string;
  className?: string;
  isDarkMode?: boolean;
  onLinkClick?: (href: string) => void;
}

// ============================================================================
// Language Mapping
// ============================================================================

const languageAliases: Record<string, string> = {
  js: 'javascript',
  ts: 'typescript',
  tsx: 'tsx',
  jsx: 'jsx',
  py: 'python',
  rb: 'ruby',
  rs: 'rust',
  go: 'go',
  java: 'java',
  kt: 'kotlin',
  cs: 'csharp',
  cpp: 'cpp',
  c: 'c',
  sh: 'bash',
  bash: 'bash',
  zsh: 'bash',
  shell: 'bash',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  xml: 'xml',
  html: 'html',
  css: 'css',
  scss: 'scss',
  sass: 'sass',
  less: 'less',
  sql: 'sql',
  md: 'markdown',
  markdown: 'markdown',
  dockerfile: 'docker',
  docker: 'docker',
  makefile: 'makefile',
  toml: 'toml',
  ini: 'ini',
  diff: 'diff',
  graphql: 'graphql',
  gql: 'graphql',
  swift: 'swift',
  php: 'php',
  lua: 'lua',
  r: 'r',
  scala: 'scala',
  elixir: 'elixir',
  erlang: 'erlang',
  clojure: 'clojure',
  haskell: 'haskell',
  ocaml: 'ocaml',
  fsharp: 'fsharp',
  vim: 'vim',
  powershell: 'powershell',
  ps1: 'powershell',
};

function normalizeLanguage(lang: string | undefined): string {
  if (!lang) return 'text';
  const lower = lang.toLowerCase();
  return languageAliases[lower] || lower;
}

// ============================================================================
// Rehype Plugin: Strip Unrecognized HTML Elements
// ============================================================================

// Standard HTML/SVG element names recognized by browsers.
// Any element not in this set will be converted to <span> by the rehype plugin
// to prevent React "unrecognized tag" warnings from LLM output containing
// XML-like tags (e.g. <settingsstate>, <bool>, <mcpmanager>).
const RECOGNIZED_HTML_TAGS = new Set([
  // HTML elements
  'a', 'abbr', 'address', 'area', 'article', 'aside', 'audio',
  'b', 'base', 'bdi', 'bdo', 'blockquote', 'body', 'br', 'button',
  'canvas', 'caption', 'cite', 'code', 'col', 'colgroup',
  'data', 'datalist', 'dd', 'del', 'details', 'dfn', 'dialog', 'div', 'dl', 'dt',
  'em', 'embed',
  'fieldset', 'figcaption', 'figure', 'footer', 'form',
  'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'head', 'header', 'hgroup', 'hr', 'html',
  'i', 'iframe', 'img', 'input', 'ins',
  'kbd',
  'label', 'legend', 'li', 'link',
  'main', 'map', 'mark', 'math', 'menu', 'meta', 'meter',
  'nav', 'noscript',
  'object', 'ol', 'optgroup', 'option', 'output',
  'p', 'param', 'picture', 'pre', 'progress',
  'q',
  'rb', 'rp', 'rt', 'rtc', 'ruby',
  's', 'samp', 'script', 'search', 'section', 'select', 'slot', 'small', 'source',
  'span', 'strong', 'style', 'sub', 'summary', 'sup', 'svg',
  'table', 'tbody', 'td', 'template', 'textarea', 'tfoot', 'th', 'thead', 'time',
  'title', 'tr', 'track',
  'u', 'ul',
  'var', 'video',
  'wbr',
  // SVG elements
  'circle', 'clippath', 'defs', 'ellipse', 'g', 'image', 'line',
  'lineargradient', 'mask', 'path', 'pattern', 'polygon', 'polyline',
  'radialgradient', 'rect', 'stop', 'text', 'tspan', 'use',
  // Deprecated but still recognized by browsers
  'big', 'center', 'font', 'nobr', 'strike', 'tt',
]);

/**
 * Rehype plugin that converts unrecognized HTML elements to <span>.
 * Runs after rehype-raw so it operates on the parsed HAST — code blocks
 * are unaffected since their content is text nodes, not element nodes.
 */
function rehypeStripUnknownElements() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (tree: any) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function walk(node: any) {
      if (node.type === 'element' && !RECOGNIZED_HTML_TAGS.has(node.tagName)) {
        node.tagName = 'span';
        node.properties = {};
      }
      if (node.children) {
        for (const child of node.children) {
          walk(child);
        }
      }
    }
    walk(tree);
  };
}

// ============================================================================
// MarkdownRenderer Component
// ============================================================================

export const MarkdownRenderer = memo(function MarkdownRenderer({
  content,
  className,
  isDarkMode = true,
  onLinkClick,
}: MarkdownRendererProps) {
  // Memoize components object to prevent re-renders
  const components: Components = useMemo(
    () => ({
      // Code blocks with syntax highlighting
      code({ className: codeClassName, children, ...props }) {
        const match = /language-(\w+)/.exec(codeClassName || '');
        const language = normalizeLanguage(match?.[1]);
        const isInline = !match && !String(children).includes('\n');

        if (isInline) {
          return (
            <code
              className={clsx(
                'px-1.5 py-0.5 rounded text-sm font-mono',
                'bg-gray-100 dark:bg-gray-700',
                'text-gray-800 dark:text-gray-200'
              )}
              {...props}
            >
              {children}
            </code>
          );
        }

        const code = String(children).replace(/\n$/, '');

        return (
          <CodeBlock
            code={code}
            language={language}
            isDarkMode={isDarkMode}
          />
        );
      },

      // Links open in external browser
      a({ href, children, ...props }) {
        const handleClick = (e: React.MouseEvent) => {
          e.preventDefault();
          if (href) {
            if (onLinkClick) {
              onLinkClick(href);
            } else {
              // Use Electron/Tauri shell to open external links
              window.open(href, '_blank', 'noopener,noreferrer');
            }
          }
        };

        return (
          <a
            href={href}
            onClick={handleClick}
            className={clsx(
              'text-primary-600 dark:text-primary-400',
              'hover:underline cursor-pointer'
            )}
            {...props}
          >
            {children}
          </a>
        );
      },

      // Images with lazy loading
      img({ src, alt, ...props }) {
        return (
          <img
            src={src}
            alt={alt || ''}
            loading="lazy"
            className="max-w-full h-auto rounded-lg my-2"
            {...props}
          />
        );
      },

      // Tables with proper styling
      table({ children, ...props }) {
        return (
          <div className="overflow-x-auto my-4">
            <table
              className={clsx(
                'min-w-full border-collapse',
                'border border-gray-200 dark:border-gray-700'
              )}
              {...props}
            >
              {children}
            </table>
          </div>
        );
      },

      thead({ children, ...props }) {
        return (
          <thead
            className="bg-gray-50 dark:bg-gray-800"
            {...props}
          >
            {children}
          </thead>
        );
      },

      th({ children, ...props }) {
        return (
          <th
            className={clsx(
              'px-4 py-2 text-left text-sm font-semibold',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-700 dark:text-gray-300'
            )}
            {...props}
          >
            {children}
          </th>
        );
      },

      td({ children, ...props }) {
        return (
          <td
            className={clsx(
              'px-4 py-2 text-sm',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-600 dark:text-gray-400'
            )}
            {...props}
          >
            {children}
          </td>
        );
      },

      // Task lists
      li({ children, ...props }) {
        // Check if this is a task list item
        const className = (props as { className?: string }).className;
        if (className?.includes('task-list-item')) {
          return (
            <li
              className="flex items-start gap-2 list-none"
              {...props}
            >
              {children}
            </li>
          );
        }

        return (
          <li className="ml-4" {...props}>
            {children}
          </li>
        );
      },

      // Task list checkbox
      input({ type, checked, ...props }) {
        if (type === 'checkbox') {
          return (
            <input
              type="checkbox"
              checked={checked}
              disabled
              className={clsx(
                'w-4 h-4 mt-1 rounded',
                'border-gray-300 dark:border-gray-600',
                'text-primary-600 dark:text-primary-400'
              )}
              {...props}
            />
          );
        }
        return <input type={type} {...props} />;
      },

      // Blockquotes
      blockquote({ children, ...props }) {
        return (
          <blockquote
            className={clsx(
              'border-l-4 border-gray-300 dark:border-gray-600',
              'pl-4 py-1 my-4',
              'text-gray-600 dark:text-gray-400 italic'
            )}
            {...props}
          >
            {children}
          </blockquote>
        );
      },

      // Horizontal rule
      hr({ ...props }) {
        return (
          <hr
            className="my-6 border-gray-200 dark:border-gray-700"
            {...props}
          />
        );
      },

      // Headers with proper styling
      h1({ children, ...props }) {
        return (
          <h1
            className="text-2xl font-bold mt-6 mb-4 text-gray-900 dark:text-white"
            {...props}
          >
            {children}
          </h1>
        );
      },

      h2({ children, ...props }) {
        return (
          <h2
            className="text-xl font-semibold mt-5 mb-3 text-gray-900 dark:text-white"
            {...props}
          >
            {children}
          </h2>
        );
      },

      h3({ children, ...props }) {
        return (
          <h3
            className="text-lg font-semibold mt-4 mb-2 text-gray-900 dark:text-white"
            {...props}
          >
            {children}
          </h3>
        );
      },

      h4({ children, ...props }) {
        return (
          <h4
            className="text-base font-semibold mt-3 mb-2 text-gray-800 dark:text-gray-100"
            {...props}
          >
            {children}
          </h4>
        );
      },

      // Paragraphs
      p({ children, ...props }) {
        return (
          <p
            className="my-2 text-gray-700 dark:text-gray-300 leading-relaxed"
            {...props}
          >
            {children}
          </p>
        );
      },

      // Lists
      ul({ children, ...props }) {
        const className = (props as { className?: string }).className;
        if (className?.includes('contains-task-list')) {
          return (
            <ul className="space-y-1 my-2" {...props}>
              {children}
            </ul>
          );
        }
        return (
          <ul className="list-disc list-outside ml-4 my-2 space-y-1" {...props}>
            {children}
          </ul>
        );
      },

      ol({ children, ...props }) {
        return (
          <ol className="list-decimal list-outside ml-4 my-2 space-y-1" {...props}>
            {children}
          </ol>
        );
      },

      // Strong and emphasis
      strong({ children, ...props }) {
        return (
          <strong className="font-semibold text-gray-900 dark:text-white" {...props}>
            {children}
          </strong>
        );
      },

      em({ children, ...props }) {
        return (
          <em className="italic" {...props}>
            {children}
          </em>
        );
      },

      // Strikethrough
      del({ children, ...props }) {
        return (
          <del className="line-through text-gray-500 dark:text-gray-400" {...props}>
            {children}
          </del>
        );
      },

      // Pre (code wrapper)
      pre({ children }) {
        // The pre tag wraps code blocks, we render just the children
        // since CodeBlock handles the wrapper
        return <>{children}</>;
      },
    }),
    [isDarkMode, onLinkClick]
  );

  // Escape tags that rehype-raw would try to render as unrecognized DOM elements.
  // SVG tags cause React warnings outside <svg> context; tool-related tags from
  // LLM output (e.g. <tool_call>, <ls>, <arg_value>) are not valid HTML elements.
  const safeContent = useMemo(() => {
    return content.replace(
      /<(\/?)(?:svg|path|circle|rect|line|polyline|polygon|ellipse|g|tool_call|tool_result|arg_key|arg_value|ls|cwd|search_results?)\b/gi,
      (_, slash) => `&lt;${slash || ''}`
    );
  }, [content]);

  return (
    <div className={clsx('markdown-content', className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm, remarkMath]}
        rehypePlugins={[rehypeKatex, rehypeRaw, rehypeStripUnknownElements]}
        components={components}
      >
        {safeContent}
      </ReactMarkdown>
    </div>
  );
});

export default MarkdownRenderer;
