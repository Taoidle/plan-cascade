/**
 * Preview Component
 *
 * Renders markdown content as HTML using GitHub Flavored Markdown (GFM).
 * Includes syntax highlighting for code blocks.
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { FileTextIcon } from '@radix-ui/react-icons';

// Import highlight.js styles - using GitHub style
import 'highlight.js/styles/github-dark.css';

interface PreviewProps {
  content: string;
  className?: string;
}

export function Preview({ content, className }: PreviewProps) {
  const { t } = useTranslation();

  // Memoize the markdown rendering for performance
  const renderedContent = useMemo(() => {
    if (!content.trim()) {
      return null;
    }

    return (
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={{
          // Custom heading components with anchor links
          h1: ({ children, ...props }) => (
            <h1 className="text-2xl font-bold mt-6 mb-4 pb-2 border-b border-gray-200 dark:border-gray-700" {...props}>
              {children}
            </h1>
          ),
          h2: ({ children, ...props }) => (
            <h2 className="text-xl font-bold mt-5 mb-3 pb-1 border-b border-gray-200 dark:border-gray-700" {...props}>
              {children}
            </h2>
          ),
          h3: ({ children, ...props }) => (
            <h3 className="text-lg font-bold mt-4 mb-2" {...props}>
              {children}
            </h3>
          ),
          h4: ({ children, ...props }) => (
            <h4 className="text-base font-bold mt-3 mb-2" {...props}>
              {children}
            </h4>
          ),
          h5: ({ children, ...props }) => (
            <h5 className="text-sm font-bold mt-2 mb-1" {...props}>
              {children}
            </h5>
          ),
          h6: ({ children, ...props }) => (
            <h6 className="text-sm font-semibold mt-2 mb-1 text-gray-600 dark:text-gray-400" {...props}>
              {children}
            </h6>
          ),

          // Paragraph
          p: ({ children, ...props }) => (
            <p className="my-3 leading-relaxed" {...props}>
              {children}
            </p>
          ),

          // Links
          a: ({ children, href, ...props }) => (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-primary-600 dark:text-primary-400 hover:underline"
              {...props}
            >
              {children}
            </a>
          ),

          // Lists
          ul: ({ children, ...props }) => (
            <ul className="my-3 ml-6 list-disc space-y-1" {...props}>
              {children}
            </ul>
          ),
          ol: ({ children, ...props }) => (
            <ol className="my-3 ml-6 list-decimal space-y-1" {...props}>
              {children}
            </ol>
          ),
          li: ({ children, ...props }) => (
            <li className="leading-relaxed" {...props}>
              {children}
            </li>
          ),

          // Task list items (GFM)
          input: ({ type, checked, ...props }) => {
            if (type === 'checkbox') {
              return (
                <input
                  type="checkbox"
                  checked={checked}
                  readOnly
                  className="mr-2 rounded border-gray-300 dark:border-gray-600"
                  {...props}
                />
              );
            }
            return <input type={type} {...props} />;
          },

          // Blockquote
          blockquote: ({ children, ...props }) => (
            <blockquote
              className="my-4 pl-4 border-l-4 border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 italic"
              {...props}
            >
              {children}
            </blockquote>
          ),

          // Code
          code: ({ className, children, ...props }) => {
            const isInline = !className;
            if (isInline) {
              return (
                <code
                  className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-sm font-mono text-red-600 dark:text-red-400"
                  {...props}
                >
                  {children}
                </code>
              );
            }
            return (
              <code className={className} {...props}>
                {children}
              </code>
            );
          },

          // Pre (code block container)
          pre: ({ children, ...props }) => (
            <pre className="my-4 p-4 rounded-lg bg-gray-900 overflow-x-auto text-sm" {...props}>
              {children}
            </pre>
          ),

          // Table (GFM)
          table: ({ children, ...props }) => (
            <div className="my-4 overflow-x-auto">
              <table
                className="min-w-full divide-y divide-gray-200 dark:divide-gray-700 border border-gray-200 dark:border-gray-700"
                {...props}
              >
                {children}
              </table>
            </div>
          ),
          thead: ({ children, ...props }) => (
            <thead className="bg-gray-50 dark:bg-gray-800" {...props}>
              {children}
            </thead>
          ),
          tbody: ({ children, ...props }) => (
            <tbody className="divide-y divide-gray-200 dark:divide-gray-700" {...props}>
              {children}
            </tbody>
          ),
          tr: ({ children, ...props }) => <tr {...props}>{children}</tr>,
          th: ({ children, ...props }) => (
            <th
              className="px-4 py-2 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider"
              {...props}
            >
              {children}
            </th>
          ),
          td: ({ children, ...props }) => (
            <td className="px-4 py-2 text-sm" {...props}>
              {children}
            </td>
          ),

          // Horizontal rule
          hr: (props) => <hr className="my-6 border-gray-200 dark:border-gray-700" {...props} />,

          // Strong and emphasis
          strong: ({ children, ...props }) => (
            <strong className="font-bold" {...props}>
              {children}
            </strong>
          ),
          em: ({ children, ...props }) => (
            <em className="italic" {...props}>
              {children}
            </em>
          ),

          // Strikethrough (GFM)
          del: ({ children, ...props }) => (
            <del className="line-through text-gray-500 dark:text-gray-400" {...props}>
              {children}
            </del>
          ),

          // Images
          img: ({ src, alt, ...props }) => (
            <img src={src} alt={alt} className="max-w-full h-auto my-4 rounded-lg" loading="lazy" {...props} />
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    );
  }, [content]);

  return (
    <div className={clsx('h-full flex flex-col', className)}>
      {/* Header */}
      <div className="px-4 py-2 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">{t('markdownEditor.preview.title')}</h3>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {renderedContent ? (
          <article className="prose prose-gray dark:prose-invert max-w-none text-gray-900 dark:text-gray-100">
            {renderedContent}
          </article>
        ) : (
          <div className="h-full flex flex-col items-center justify-center text-gray-400">
            <FileTextIcon className="w-12 h-12 mb-2" />
            <p className="text-sm">{t('markdownEditor.preview.empty')}</p>
          </div>
        )}
      </div>
    </div>
  );
}

export default Preview;
