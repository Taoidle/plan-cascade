/**
 * ToolCallStreamFilter
 *
 * A state-machine that processes streaming text chunks and filters out
 * tool_call blocks emitted by LLMs using the FallbackToolFormatMode.
 *
 * Blocks matching these patterns are suppressed:
 *   ```tool_call\n{"tool": "...", ...}\n```       (fenced)
 *   ```json\n{"tool": "...", ...}\n```             (fenced json)
 *   tool_call\n{"tool": "...", ...}                (bare, no fences)
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

/**
 * Check if a line (trimmed) is a bare tool_call marker.
 * Matches: "tool_call", "tool_call:", "tool_call :" (with optional whitespace)
 */
function isBareToolCallLine(trimmedLine: string): boolean {
  return /^tool_call\s*:?\s*$/.test(trimmedLine);
}

export class ToolCallStreamFilter {
  private state: State = State.NORMAL;
  private buffer = '';
  /** Tracks whether we saw a tool name in the suppressed block */
  private toolName = '';
  /**
   * Buffer for a bare "tool_call" line detected at the end of a chunk.
   * On the next chunk, if the continuation starts with "{", both the
   * buffered line and the JSON line are suppressed (bare tool call pattern).
   * Otherwise the buffer is flushed as normal text.
   */
  private pendingBareToolCall = '';

  /**
   * Process a streaming text chunk.
   *
   * Returns the text that should be displayed to the user (output)
   * and optionally a tool indicator when a tool block was fully consumed.
   */
  processChunk(text: string): FilterResult {
    let input = text;
    let prefixOutput = '';

    // --- Handle pending bare tool_call from previous chunk ---
    if (this.pendingBareToolCall) {
      const trimmedStart = input.trimStart();
      if (trimmedStart.startsWith('{')) {
        // Confirmed bare tool call with JSON -- suppress the { line too
        const bracePos = input.indexOf('{');
        const newlineAfterBrace = input.indexOf('\n', bracePos);
        if (newlineAfterBrace !== -1) {
          // Suppress up to end of the { line, continue with rest
          input = input.slice(newlineAfterBrace + 1);
        } else {
          // The { line extends to end of chunk -- suppress everything
          this.pendingBareToolCall = '';
          return { output: '' };
        }
      }
      // Bare "tool_call" lines are always LLM tool-calling syntax —
      // suppress them regardless of what follows (even without a JSON
      // body, e.g. when followed by a blank line then a fenced block).
      this.pendingBareToolCall = '';
    }

    // --- Run the main state machine ---
    let output = '';
    let toolIndicator: string | undefined;
    let i = 0;

    while (i < input.length) {
      switch (this.state) {
        case State.NORMAL: {
          // Look for opening triple-backtick
          const fenceIdx = input.indexOf('```', i);
          if (fenceIdx === -1) {
            // No fence in remaining text
            output += input.slice(i);
            i = input.length;
          } else {
            // Output everything before the fence
            output += input.slice(i, fenceIdx);
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
          const closingIdx = input.indexOf('```', i);

          // Grab text up to either the closing fence or end of chunk
          const segment = closingIdx === -1 ? input.slice(i) : input.slice(i, closingIdx);
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
          const closeIdx = input.indexOf('```', i);
          if (closeIdx === -1) {
            // No closing fence yet; buffer everything (for tool name extraction)
            this.buffer += input.slice(i);
            i = input.length;
          } else {
            // Found closing fence
            this.buffer += input.slice(i, closeIdx);
            this.extractToolName(this.buffer);
            i = closeIdx + 3;
            const result = this.completeToolBlock();
            toolIndicator = result;
          }
          break;
        }
      }
    }

    // --- Post-processing: strip inline bare tool_call patterns ---
    if (this.state === State.NORMAL && output) {
      output = this.stripInlineBareToolCalls(output);
      output = this.bufferTrailingBareToolCall(output);
    }

    output = prefixOutput + output;

    return { output, toolIndicator };
  }

  /**
   * Flush any buffered content (e.g., when the stream ends).
   * If we're in MAYBE_BLOCK, the incomplete fence is returned as normal text.
   * If we're in IN_TOOL_BLOCK, the buffered content is suppressed (incomplete tool block).
   */
  flush(): string {
    // Suppress any pending bare tool_call — it's LLM syntax, not user text.
    this.pendingBareToolCall = '';

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
    this.pendingBareToolCall = '';
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

  /**
   * Strip bare tool_call patterns that appear entirely within a single chunk.
   * Matches: \ntool_call\n{...\n  (newline-delimited bare pattern)
   */
  private stripInlineBareToolCalls(text: string): string {
    // Pattern: newline + "tool_call" (optional whitespace/colon) + newline + {-line
    let result = text.replace(/\ntool_call\s*:?\s*\n\{[^\n]*/g, '');
    // Also match at start of string
    result = result.replace(/^tool_call\s*:?\s*\n\{[^\n]*/, '');
    // Strip bare "tool_call" lines even without a following "{" — these
    // are always LLM tool-calling syntax, not user-visible content.
    // Only strip mid-text occurrences (followed by \n); trailing ones are
    // handled by bufferTrailingBareToolCall for cross-chunk detection.
    // Use [^\S\n] instead of \s to avoid consuming the newline delimiter.
    result = result.replace(/\ntool_call[^\S\n]*:?[^\S\n]*(?=\n)/g, '');
    result = result.replace(/^tool_call[^\S\n]*:?[^\S\n]*(?=\n)/, '');
    return result;
  }

  /**
   * If the output ends with a line that is just "tool_call" (or "tool_call:"),
   * buffer it. On the next chunk, we'll check if it's followed by "{".
   */
  private bufferTrailingBareToolCall(text: string): string {
    const lastNewlineIdx = text.lastIndexOf('\n');
    const lastLine = lastNewlineIdx === -1 ? text : text.slice(lastNewlineIdx + 1);
    const trimmedLastLine = lastLine.trim();

    if (isBareToolCallLine(trimmedLastLine)) {
      if (lastNewlineIdx === -1) {
        // Entire output is just "tool_call" -- buffer it all
        this.pendingBareToolCall = text;
        return '';
      }
      // Buffer from the last newline (inclusive)
      this.pendingBareToolCall = text.slice(lastNewlineIdx);
      return text.slice(0, lastNewlineIdx);
    }

    return text;
  }
}
