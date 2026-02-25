import { describe, it, expect, beforeEach } from 'vitest';
import { ToolCallStreamFilter } from './toolCallFilter';

describe('ToolCallStreamFilter', () => {
  let filter: ToolCallStreamFilter;

  beforeEach(() => {
    filter = new ToolCallStreamFilter();
  });

  // =========================================================================
  // 1. Single complete tool_call block -- should be filtered, returns toolIndicator
  // =========================================================================
  describe('single complete tool_call block', () => {
    it('filters a complete tool_call block and returns a tool indicator', () => {
      const input = '```tool_call\n{"tool": "Read", "params": {"path": "src/main.rs"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe('');
      expect(result.toolIndicator).toBe('[tool_call] Read');
    });

    it('filters a tool_call block with extra whitespace', () => {
      const input = '```tool_call\n  {"tool": "Write", "params": {"path": "out.txt", "content": "hello"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe('');
      expect(result.toolIndicator).toBe('[tool_call] Write');
    });
  });

  // =========================================================================
  // 2. Normal code block (```python) -- should pass through unchanged
  // =========================================================================
  describe('normal code blocks pass through', () => {
    it('passes through a python code block', () => {
      const input = '```python\nprint("hello world")\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('passes through a rust code block', () => {
      const input = '```rust\nfn main() { println!("hello"); }\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('passes through a typescript code block', () => {
      const input = '```typescript\nconst x: number = 42;\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('passes through a bash code block', () => {
      const input = '```bash\necho "hello"\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });
  });

  // =========================================================================
  // 3. Block split across 2 chunks
  // =========================================================================
  describe('block split across chunks', () => {
    it('handles tool_call tag split across two chunks', () => {
      const r1 = filter.processChunk('```tool_ca');
      // Should buffer -- no output yet
      expect(r1.output).toBe('');
      expect(r1.toolIndicator).toBeUndefined();

      const r2 = filter.processChunk('ll\n{"tool": "Grep", "params": {"pattern": "foo"}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Grep');
    });

    it('handles opening fence in one chunk, rest in another', () => {
      const r1 = filter.processChunk('Some text before ```');
      expect(r1.output).toBe('Some text before ');
      expect(r1.toolIndicator).toBeUndefined();

      const r2 = filter.processChunk('tool_call\n{"tool": "Bash", "params": {"command": "ls"}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Bash');
    });

    it('handles block body split across multiple chunks', () => {
      const r1 = filter.processChunk('```tool_call\n{"tool": "Read"');
      expect(r1.output).toBe('');
      expect(r1.toolIndicator).toBeUndefined();

      const r2 = filter.processChunk(', "params": {"path": "file.txt"}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Read');
    });

    it('handles fence markers split across chunks', () => {
      // Opening ``` split: first chunk ends with "`", second starts with "``tool_call..."
      // Actually, the scanner looks for "```" as a substring, so a partial ` at end
      // just passes through. Let's test a more realistic split.
      const r1 = filter.processChunk('Hello ```tool_call\n{"tool": "Edit"');
      expect(r1.output).toBe('Hello ');

      const r2 = filter.processChunk(', "params": {}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Edit');
    });
  });

  // =========================================================================
  // 4. Multiple consecutive tool_call blocks -- all filtered
  // =========================================================================
  describe('multiple consecutive tool_call blocks', () => {
    it('filters two consecutive tool_call blocks', () => {
      const block1 = '```tool_call\n{"tool": "Read", "params": {"path": "a.rs"}}\n```';
      const block2 = '```tool_call\n{"tool": "Write", "params": {"path": "b.rs", "content": "x"}}\n```';
      const input = block1 + block2;

      const result = filter.processChunk(input);
      expect(result.output).toBe('');
      // The last toolIndicator wins (processChunk returns the last one encountered)
      expect(result.toolIndicator).toBe('[tool_call] Write');
    });

    it('filters three blocks delivered in separate chunks', () => {
      const r1 = filter.processChunk('```tool_call\n{"tool": "Read", "params": {}}\n```');
      expect(r1.output).toBe('');
      expect(r1.toolIndicator).toBe('[tool_call] Read');

      const r2 = filter.processChunk('```tool_call\n{"tool": "Grep", "params": {}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Grep');

      const r3 = filter.processChunk('```tool_call\n{"tool": "Bash", "params": {}}\n```');
      expect(r3.output).toBe('');
      expect(r3.toolIndicator).toBe('[tool_call] Bash');
    });
  });

  // =========================================================================
  // 5. Mixed content: normal text, then tool_call block, then more text
  // =========================================================================
  describe('mixed content', () => {
    it('preserves text around a tool_call block', () => {
      const input =
        'Let me read the file. ```tool_call\n{"tool": "Read", "params": {"path": "x.ts"}}\n```Here are the results.';
      const result = filter.processChunk(input);
      expect(result.output).toBe('Let me read the file. Here are the results.');
      expect(result.toolIndicator).toBe('[tool_call] Read');
    });

    it('preserves text when tool block is in separate chunk', () => {
      const r1 = filter.processChunk('Before the block ');
      expect(r1.output).toBe('Before the block ');

      const r2 = filter.processChunk('```tool_call\n{"tool": "Read", "params": {}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Read');

      const r3 = filter.processChunk(' After the block');
      expect(r3.output).toBe(' After the block');
      expect(r3.toolIndicator).toBeUndefined();
    });

    it('handles text, code block, tool block, text sequence', () => {
      const r1 = filter.processChunk(
        'Normal text\n```python\nprint("hi")\n```\nMore text ```tool_call\n{"tool": "Bash", "params": {}}\n```\nFinal text',
      );
      expect(r1.output).toBe('Normal text\n```python\nprint("hi")\n```\nMore text \nFinal text');
      expect(r1.toolIndicator).toBe('[tool_call] Bash');
    });
  });

  // =========================================================================
  // 6. Empty content -- handled gracefully
  // =========================================================================
  describe('empty content', () => {
    it('handles empty string', () => {
      const result = filter.processChunk('');
      expect(result.output).toBe('');
      expect(result.toolIndicator).toBeUndefined();
    });

    it('handles whitespace-only string', () => {
      const result = filter.processChunk('   \n\t  ');
      expect(result.output).toBe('   \n\t  ');
      expect(result.toolIndicator).toBeUndefined();
    });
  });

  // =========================================================================
  // 7. Block with ```json containing tool-like JSON -- filtered
  // =========================================================================
  describe('json blocks with tool-like content', () => {
    it('filters a json block containing tool call JSON', () => {
      const input = '```json\n{"tool": "Read", "params": {"path": "src/lib.rs"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe('');
      expect(result.toolIndicator).toBe('[tool_call] Read');
    });

    it('filters a json block with tool_name key', () => {
      const input = '```json\n{"tool_name": "Write", "arguments": {"path": "out.txt"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe('');
      // tool_name doesn't match our "tool" key extraction, but the block should still be filtered
      // since it matches the tool-like pattern (starts with {"tool)
      expect(result.toolIndicator).toBeDefined();
    });
  });

  // =========================================================================
  // 8. Block with ```json containing non-tool JSON -- passed through
  // =========================================================================
  describe('json blocks with non-tool content', () => {
    it('passes through a json block with regular JSON data', () => {
      const input = '```json\n{"name": "John", "age": 30}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('passes through a json block with array data', () => {
      const input = '```json\n[1, 2, 3]\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('passes through a json block with config-like data', () => {
      const input = '```json\n{"compilerOptions": {"target": "ES2020"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });
  });

  // =========================================================================
  // 9. flush() returns buffered content when in MAYBE_BLOCK state
  // =========================================================================
  describe('flush()', () => {
    it('returns buffered content when stream ends in MAYBE_BLOCK state', () => {
      // Feed a partial opening fence
      filter.processChunk('Some text ```');
      const flushed = filter.flush();
      // Should return the buffered opening fence
      expect(flushed).toBe('```');
    });

    it('returns empty string when in NORMAL state', () => {
      filter.processChunk('Normal text without any fences');
      const flushed = filter.flush();
      expect(flushed).toBe('');
    });

    it('returns empty string when in IN_TOOL_BLOCK state (suppress incomplete)', () => {
      // Feed confirmed tool block without closing fence
      filter.processChunk('```tool_call\n{"tool": "Read", "params": {}}');
      const flushed = filter.flush();
      expect(flushed).toBe('');
    });

    it('returns partial fence tag when stream ends mid-classification', () => {
      filter.processChunk('```to');
      const flushed = filter.flush();
      // Should return the buffered content since we couldn't classify
      // 'to' is a prefix of 'tool_call', so classification is 'pending'
      expect(flushed).toBe('```to');
    });
  });

  // =========================================================================
  // Additional edge cases
  // =========================================================================
  describe('reset()', () => {
    it('resets state for a new execution turn', () => {
      // Put filter in a mid-block state
      filter.processChunk('```tool_call\n{"tool": "Read"');
      filter.reset();

      // After reset, should be in NORMAL state
      const result = filter.processChunk('Normal text');
      expect(result.output).toBe('Normal text');
      expect(result.toolIndicator).toBeUndefined();
    });

    it('flush returns empty after reset', () => {
      filter.processChunk('```tool_ca');
      filter.reset();
      expect(filter.flush()).toBe('');
    });
  });

  describe('edge cases', () => {
    it('handles triple backticks not at start of text', () => {
      const input = 'Here is the code: ```python\nprint("hi")\n``` and that is it.';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('handles text with backticks that are not triple', () => {
      const input = 'Use `code` inline and ``double`` too.';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('handles a tool_call block immediately followed by normal text', () => {
      const r1 = filter.processChunk(
        '```tool_call\n{"tool": "Glob", "params": {"pattern": "*.ts"}}\n```The results are:',
      );
      expect(r1.output).toBe('The results are:');
      expect(r1.toolIndicator).toBe('[tool_call] Glob');
    });
  });

  // =========================================================================
  // Story-005: Gap-filling edge case tests
  // =========================================================================
  describe('untagged code fence behavior', () => {
    it('passes through an untagged fence block even with tool-like JSON (by design)', () => {
      // The filter only catches ```tool_call and ```json{tool} blocks.
      // Untagged fences are NOT filtered because they are ambiguous.
      const input = '```\n{"tool": "Read", "params": {"path": "src/main.rs"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });

    it('passes through an untagged fence block with non-tool content', () => {
      const input = '```\nsome random text content\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });
  });

  describe('closing fence split across chunks', () => {
    it('handles closing ``` arriving in a separate chunk after tool block', () => {
      const r1 = filter.processChunk('```tool_call\n{"tool": "Write", "params": {"path": "out.txt"}}');
      expect(r1.output).toBe('');
      expect(r1.toolIndicator).toBeUndefined();

      const r2 = filter.processChunk('\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Write');
    });

    it('handles body arriving in multiple small chunks before closing fence', () => {
      const r1 = filter.processChunk('```tool_call\n{"tool": "Bash",');
      expect(r1.output).toBe('');

      const r2 = filter.processChunk(' "params": {"command": "ls -la"}}');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBeUndefined();

      // Closing fence arrives in a single chunk (scanner requires ``` to be contiguous)
      const r3 = filter.processChunk('\n```');
      expect(r3.output).toBe('');
      expect(r3.toolIndicator).toBe('[tool_call] Bash');
    });
  });

  describe('partial classification pending states', () => {
    it('buffers when chunk ends with partial "j" that could be "json"', () => {
      const r1 = filter.processChunk('```j');
      // "j" is a prefix of "json", so classification is pending
      expect(r1.output).toBe('');
      expect(r1.toolIndicator).toBeUndefined();

      // Complete with "son" + tool JSON
      const r2 = filter.processChunk('son\n{"tool": "Read", "params": {"path": "a.rs"}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Read');
    });

    it('buffers when chunk ends with partial "to" that could be "tool_call"', () => {
      const r1 = filter.processChunk('```to');
      expect(r1.output).toBe('');

      // Complete with "ol_call" + content
      const r2 = filter.processChunk('ol_call\n{"tool": "Grep", "params": {"pattern": "fn"}}\n```');
      expect(r2.output).toBe('');
      expect(r2.toolIndicator).toBe('[tool_call] Grep');
    });
  });

  describe('multiple tool indicators in single chunk', () => {
    it('returns the last tool indicator when multiple blocks are in one chunk', () => {
      const input =
        '```tool_call\n{"tool": "Read", "params": {}}\n```' +
        'middle text' +
        '```tool_call\n{"tool": "Write", "params": {}}\n```' +
        'end text';
      const result = filter.processChunk(input);
      // Only the last toolIndicator is returned per processChunk contract
      expect(result.toolIndicator).toBe('[tool_call] Write');
      expect(result.output).toBe('middle textend text');
    });
  });

  describe('tool_name key variant extraction', () => {
    it('extracts tool name from "tool_name" key in json block', () => {
      // tool_name matches /"tool/ regex, so it should be recognized
      const input = '```json\n{"tool_name": "Edit", "arguments": {"path": "f.ts"}}\n```';
      const result = filter.processChunk(input);
      expect(result.output).toBe('');
      // The extractToolName regex looks for "tool": specifically, so tool_name won't match
      // But the block should still be filtered (toolIndicator defaults to 'unknown')
      expect(result.toolIndicator).toBe('[tool_call] unknown');
    });
  });

  describe('filter state across complete lifecycle', () => {
    it('correctly handles reset between execution turns', () => {
      // First turn: partial block
      filter.processChunk('```tool_call\n{"tool": "Read"');
      // Reset for new turn
      filter.reset();

      // Second turn: completely new content
      const r = filter.processChunk('Normal text without any fences');
      expect(r.output).toBe('Normal text without any fences');
      expect(r.toolIndicator).toBeUndefined();
    });

    it('correctly handles flush then new content', () => {
      // Start a tool block without closing
      filter.processChunk('```tool_call\n{"tool": "Read", "params": {}}');
      const flushed = filter.flush();
      expect(flushed).toBe('');

      // After flush, filter should be in NORMAL state
      const r = filter.processChunk('New text after flush');
      expect(r.output).toBe('New text after flush');
      expect(r.toolIndicator).toBeUndefined();
    });

    it('handles flush of MAYBE_BLOCK then new tool block', () => {
      // Start an ambiguous fence
      filter.processChunk('Some text ```');
      const flushed = filter.flush();
      expect(flushed).toBe('```');

      // After flush, process a real tool block
      const r = filter.processChunk('```tool_call\n{"tool": "Bash", "params": {}}\n```');
      expect(r.output).toBe('');
      expect(r.toolIndicator).toBe('[tool_call] Bash');
    });
  });

  // =========================================================================
  // Bare tool_call patterns (no backtick fences)
  // =========================================================================
  describe('bare tool_call patterns (no fences)', () => {
    it('strips inline bare tool_call\\n{ pattern within a single chunk', () => {
      const input = 'Let me check the code.\ntool_call\n{"tool": "Cwd"}\nHere is the result.';
      const result = filter.processChunk(input);
      expect(result.output).toBe('Let me check the code.\nHere is the result.');
    });

    it('strips bare tool_call at start of chunk', () => {
      const input = 'tool_call\n{"tool": "LS"}\nSome text after.';
      const result = filter.processChunk(input);
      expect(result.output).toBe('\nSome text after.');
    });

    it('buffers trailing bare tool_call and suppresses on next chunk with {', () => {
      const r1 = filter.processChunk('Some explanation text\ntool_call');
      expect(r1.output).toBe('Some explanation text');

      const r2 = filter.processChunk('\n{"tool": "Read"}\nContinuing...');
      expect(r2.output).toBe('Continuing...');
    });

    it('suppresses trailing bare tool_call even when next chunk does not start with {', () => {
      const r1 = filter.processChunk('Some text\ntool_call');
      expect(r1.output).toBe('Some text');

      // Bare "tool_call" lines are always LLM syntax — suppress them
      // regardless of what follows (e.g. blank line then fenced block)
      const r2 = filter.processChunk('\nActually, let me try something else.');
      expect(r2.output).toBe('\nActually, let me try something else.');
    });

    it('handles bare tool_call with colon variant', () => {
      const input = 'Text before\ntool_call:\n{"tool": "Grep"}\nText after';
      const result = filter.processChunk(input);
      expect(result.output).toBe('Text before\nText after');
    });

    it('does not strip tool_call in middle of a sentence', () => {
      const input = 'The tool_call mechanism allows the LLM to invoke tools.';
      const result = filter.processChunk(input);
      expect(result.output).toBe(input);
    });

    it('handles bare tool_call with only partial JSON (incomplete)', () => {
      const input = 'Explanation\ntool_call\n{"';
      const result = filter.processChunk(input);
      expect(result.output).toBe('Explanation');
    });

    it('suppresses pending bare tool_call on flush()', () => {
      filter.processChunk('Some text\ntool_call');
      const flushed = filter.flush();
      // Bare "tool_call" is suppressed even when stream ends
      expect(flushed).toBe('');
    });

    it('resets pending bare tool_call on reset()', () => {
      filter.processChunk('Text\ntool_call');
      filter.reset();
      const r = filter.processChunk('Normal text');
      expect(r.output).toBe('Normal text');
    });

    it('handles multiple bare tool_call patterns in same chunk', () => {
      const input = 'First\ntool_call\n{"tool": "A"}\nMiddle\ntool_call\n{"tool": "B"}\nEnd';
      const result = filter.processChunk(input);
      expect(result.output).toBe('First\nMiddle\nEnd');
    });

    it('suppresses bare tool_call\\n{ when { is on a new chunk', () => {
      const r1 = filter.processChunk('tool_call');
      expect(r1.output).toBe('');

      const r2 = filter.processChunk('{"incomplete json');
      expect(r2.output).toBe('');
    });

    it('suppresses bare tool_call followed by blank line then fenced block', () => {
      // Real-world pattern: LLM writes "tool_call" header, blank line,
      // then a fenced block — the bare "tool_call" should be suppressed
      const r1 = filter.processChunk('Some analysis text\ntool_call');
      expect(r1.output).toBe('Some analysis text');

      const r2 = filter.processChunk('\n\n```tool_call\n{"tool": "LS", "tool_input": {"path": "."}}\n```');
      // The bare "tool_call" is suppressed, the fenced block is filtered
      expect(r2.output).toBe('\n\n');
      expect(r2.toolIndicator).toBe('[tool_call] LS');
    });

    it('strips standalone bare tool_call line within a chunk (no JSON after)', () => {
      const input = 'Text before\ntool_call\n\nMore text after';
      const result = filter.processChunk(input);
      // \ntool_call is stripped; the \n after it (from the blank line) remains
      expect(result.output).toBe('Text before\n\nMore text after');
    });
  });

  // =========================================================================
  // MAYBE_BLOCK pending+closing-fence edge cases (feature-004 story-002)
  // =========================================================================
  describe('MAYBE_BLOCK pending+closing-fence prefix leak fix', () => {
    it('does not leak ``` artifacts for empty fence block', () => {
      const input = '```\n```';
      const result = filter.processChunk(input);
      // Empty fence block should either be suppressed or passed through cleanly
      // but NOT produce orphaned ``` artifacts
      const hasOrphanedFence = result.output === '```' || result.output === '``` ```';
      expect(hasOrphanedFence).toBe(false);
    });

    it('does not leak artifacts for whitespace-only fence block', () => {
      const input = '```  \n  ```';
      const result = filter.processChunk(input);
      // Should not produce orphaned ``` artifacts
      const hasOrphanedFence = result.output === '```' || result.output === '``` ```';
      expect(hasOrphanedFence).toBe(false);
    });

    it('passes through very short content between fences as complete block', () => {
      const input = '```\na\n```';
      const result = filter.processChunk(input);
      // Short non-empty content should pass through as a complete code block
      expect(result.output).toBe('```\na\n```');
    });

    it('handles pending classification with closing fence in separate chunk cleanly', () => {
      const r1 = filter.processChunk('```\n');
      // First chunk: pending classification, buffered
      expect(r1.output).toBe('');

      const r2 = filter.processChunk('```');
      // Second chunk contains closing fence — should produce clean output
      const combined = r1.output + r2.output;
      const hasOrphanedFence = combined === '```' || combined.trim() === '```';
      // Should not be just an orphaned fence
      expect(hasOrphanedFence).toBe(false);
    });

    it('short code blocks like ```x\\ncode\\n``` are unaffected', () => {
      const input = '```x\ncode\n```';
      const result = filter.processChunk(input);
      // 'x' is not a known language and not a prefix of 'tool_call' or 'json',
      // so it classifies as 'not_tool' and passes through
      expect(result.output).toBe(input);
      expect(result.toolIndicator).toBeUndefined();
    });
  });
});
