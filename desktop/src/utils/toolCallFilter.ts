/**
 * ToolCallStreamFilter
 *
 * A state-machine that processes streaming text chunks and filters out
 * triple-backtick tool_call / tool-JSON code blocks emitted by LLMs
 * using the FallbackToolFormatMode.
 *
 * Blocks matching these patterns are suppressed:
 *   ```tool_call\n{"tool": "...", ...}\n```
 *   ```json\n{"tool": "...", ...}\n```
 *
 * All other code blocks (```python, ```rust, etc.) pass through unchanged.
 */

export interface FilterResult {
  /** Text to display to the user (may be empty string) */
  output: string;
  /** If a tool_call block was fully consumed, a short indicator string */
  toolIndicator?: string;
}

const enum State {
  /** Normal pass-through mode */
  NORMAL = 'NORMAL',
  /** Saw opening ```, waiting to determine language tag */
  MAYBE_BLOCK = 'MAYBE_BLOCK',
  /** Confirmed tool_call / json-tool block, suppressing content */
  IN_TOOL_BLOCK = 'IN_TOOL_BLOCK',
}

/**
 * Checks whether text that follows a code-fence opening looks like it
 * introduces a tool-call block.
 *
 * Recognized fence headers (after the triple-backtick):
 *   tool_call
 *   json  (only when followed by tool-like JSON content)
 *
 * Returns:
 *   'tool'       -- confirmed tool block
 *   'not_tool'   -- confirmed non-tool block (other language)
 *   'pending'    -- not enough data yet to decide
 */
function classifyFenceContent(afterFence: string): 'tool' | 'not_tool' | 'pending' {
  // Strip leading whitespace/newlines for classification but check for language tag
  const trimmed = afterFence.trimStart();

  if (trimmed.length === 0) {
    return 'pending';
  }

  // Check for explicit tool_call tag
  if (trimmed.startsWith('tool_call')) {
    return 'tool';
  }

  // Check for known non-tool language tags
  const knownLangs = [
    'python', 'py', 'rust', 'typescript', 'javascript', 'java', 'go', 'ruby',
    'c', 'cpp', 'csharp', 'cs', 'swift', 'kotlin', 'scala', 'php',
    'html', 'css', 'scss', 'less', 'sql', 'bash', 'shell', 'sh', 'zsh',
    'powershell', 'yaml', 'yml', 'toml', 'xml', 'markdown', 'md',
    'dockerfile', 'makefile', 'cmake', 'lua', 'perl', 'r', 'haskell',
    'elixir', 'erlang', 'clojure', 'ocaml', 'fsharp', 'dart', 'zig',
    'nim', 'v', 'tsx', 'jsx', 'vue', 'svelte', 'astro', 'graphql',
    'proto', 'protobuf', 'diff', 'ini', 'conf', 'cfg', 'env', 'text',
    'txt', 'log', 'csv',
  ];

  for (const lang of knownLangs) {
    if (trimmed.startsWith(lang)) {
      // Make sure it's a full match (followed by newline, space, or end)
      const afterLang = trimmed.slice(lang.length);
      if (afterLang.length === 0 || afterLang[0] === '\n' || afterLang[0] === '\r' || afterLang[0] === ' ') {
        return 'not_tool';
      }
    }
  }

  // Check for json tag -- this is ambiguous; we need to see if the body
  // contains tool-like JSON
  if (trimmed.startsWith('json')) {
    const afterTag = trimmed.slice(4);
    if (afterTag.length === 0) {
      return 'pending'; // need more data after "json"
    }
    // After "json" we expect a newline then content
    const newlineIdx = afterTag.indexOf('\n');
    if (newlineIdx === -1) {
      // No newline yet; if it's just whitespace, still pending
      if (afterTag.trim().length === 0) {
        return 'pending';
      }
      // Non-whitespace non-newline after "json" -- unusual, treat as not tool
      return 'not_tool';
    }
    // We have content after the json tag newline
    const body = afterTag.slice(newlineIdx + 1).trimStart();
    if (body.length === 0) {
      return 'pending';
    }
    // Check if the body looks like a tool call JSON object
    if (looksLikeToolJson(body)) {
      return 'tool';
    }
    return 'not_tool';
  }

  // If we see a newline, this is an untagged fence — check if body is tool-like
  if (trimmed.startsWith('\n') || trimmed.startsWith('\r')) {
    const body = trimmed.replace(/^[\r\n]+/, '').trimStart();
    if (body.length === 0) {
      return 'pending';
    }
    if (looksLikeToolJson(body)) {
      return 'tool';
    }
    return 'not_tool';
  }

  // Partial text that doesn't match any known tag yet
  // Check if it could be the start of 'tool_call' or 'json'
  if ('tool_call'.startsWith(trimmed) || 'json'.startsWith(trimmed)) {
    return 'pending';
  }

  // Some other language tag we don't explicitly know -- not a tool
  return 'not_tool';
}

/**
 * Heuristic: does this text look like the start of a tool-call JSON object?
 * Looks for patterns like: {"tool": or {"tool_name":
 */
function looksLikeToolJson(text: string): boolean {
  const trimmed = text.trimStart();
  // Match opening brace followed by a "tool" key
  if (!trimmed.startsWith('{')) return false;
  // Check for "tool" key patterns
  return /^\{\s*"tool/.test(trimmed);
}

export class ToolCallStreamFilter {
  private state: State = State.NORMAL;
  private buffer = '';
  /** Tracks whether we saw a tool name in the suppressed block */
  private toolName = '';

  /**
   * Process a streaming text chunk.
   *
   * Returns the text that should be displayed to the user (output)
   * and optionally a tool indicator when a tool block was fully consumed.
   */
  processChunk(text: string): FilterResult {
    let output = '';
    let toolIndicator: string | undefined;
    let i = 0;

    while (i < text.length) {
      switch (this.state) {
        case State.NORMAL: {
          // Look for opening triple-backtick
          const fenceIdx = text.indexOf('```', i);
          if (fenceIdx === -1) {
            // No fence in remaining text
            output += text.slice(i);
            i = text.length;
          } else {
            // Output everything before the fence
            output += text.slice(i, fenceIdx);
            // Transition to MAYBE_BLOCK, buffer the opening fence
            this.state = State.MAYBE_BLOCK;
            this.buffer = '```';
            i = fenceIdx + 3;
          }
          break;
        }

        case State.MAYBE_BLOCK: {
          // We're buffering content after an opening ``` to determine
          // if this is a tool block or a normal code block.
          // We need to consume enough text to classify the fence.

          // Look for a closing fence that would end this block
          const closingIdx = text.indexOf('```', i);

          // Grab text up to either the closing fence or end of chunk
          const segment = closingIdx === -1 ? text.slice(i) : text.slice(i, closingIdx);
          this.buffer += segment;
          i += segment.length;

          // Try to classify what we have so far
          const afterFence = this.buffer.slice(3); // text after opening ```
          const classification = classifyFenceContent(afterFence);

          if (classification === 'tool') {
            // Confirmed tool block
            this.state = State.IN_TOOL_BLOCK;
            this.extractToolName(afterFence);

            // If we also found a closing fence in this chunk, consume it
            if (closingIdx !== -1) {
              i = closingIdx + 3;
              // Block is complete
              const result = this.completeToolBlock();
              toolIndicator = result;
              // Continue processing rest of chunk in NORMAL state
            }
            // else: still in IN_TOOL_BLOCK, waiting for closing fence
          } else if (classification === 'not_tool') {
            // Not a tool block -- flush buffer as normal output
            if (closingIdx !== -1) {
              // Include closing fence and advance past it
              output += this.buffer + '```';
              i = closingIdx + 3;
            } else {
              // No closing fence yet; output what we have
              output += this.buffer;
            }
            this.buffer = '';
            this.state = State.NORMAL;
          } else {
            // 'pending' -- need more data
            // If there was a closing fence, we have enough context.
            // Since it's still pending but we've hit a closing fence,
            // the block is very short -- likely not a tool call.
            if (closingIdx !== -1) {
              // Include the closing fence in the buffer for output
              this.buffer += '```';
              output += this.buffer;
              this.buffer = '';
              this.state = State.NORMAL;
              i = closingIdx + 3;
            }
            // else: wait for more chunks
          }
          break;
        }

        case State.IN_TOOL_BLOCK: {
          // Suppress content, look for closing triple-backtick
          const closeIdx = text.indexOf('```', i);
          if (closeIdx === -1) {
            // No closing fence yet; buffer everything (for tool name extraction)
            this.buffer += text.slice(i);
            i = text.length;
          } else {
            // Found closing fence
            this.buffer += text.slice(i, closeIdx);
            this.extractToolName(this.buffer);
            i = closeIdx + 3;
            const result = this.completeToolBlock();
            toolIndicator = result;
          }
          break;
        }
      }
    }

    return { output, toolIndicator };
  }

  /**
   * Flush any buffered content (e.g., when the stream ends).
   * If we're in MAYBE_BLOCK, the incomplete fence is returned as normal text.
   * If we're in IN_TOOL_BLOCK, the buffered content is suppressed (incomplete tool block).
   */
  flush(): string {
    const buffered = this.buffer;
    this.buffer = '';

    switch (this.state) {
      case State.MAYBE_BLOCK:
        // Incomplete classification -- return as normal text
        this.state = State.NORMAL;
        return buffered;
      case State.IN_TOOL_BLOCK:
        // Incomplete tool block -- suppress
        this.state = State.NORMAL;
        return '';
      default:
        return '';
    }
  }

  /**
   * Reset the filter state for a new execution turn.
   */
  reset(): void {
    this.state = State.NORMAL;
    this.buffer = '';
    this.toolName = '';
  }

  /** Extract tool name from buffered content for the indicator */
  private extractToolName(text: string): void {
    if (this.toolName) return;
    const match = text.match(/"tool"\s*:\s*"([^"]+)"/);
    if (match) {
      this.toolName = match[1];
    }
  }

  /** Finalize a tool block: reset state and return the indicator string */
  private completeToolBlock(): string {
    const name = this.toolName || 'unknown';
    this.state = State.NORMAL;
    this.buffer = '';
    this.toolName = '';
    return `[tool_call] ${name}`;
  }
}
