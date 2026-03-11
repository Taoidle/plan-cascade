import { invoke } from '@tauri-apps/api/core';
import { mcpApi } from './mcpApi';
import type { CommandResponse } from './tauri';
import type { ConnectedMcpToolDetail } from '../types/mcp';
import type { DebugBrowserBridgeStatus } from '../types/debugMode';

interface BrowserAvailability {
  feature_compiled: boolean;
  browser_detected: boolean;
  browser_path?: string | null;
}

interface BrowserActionResult {
  success: boolean;
  output?: string | null;
  current_url?: string | null;
  page_title?: string | null;
}

type JsonRecord = Record<string, unknown>;

type BrowserActionPayload =
  | { action: 'open_page'; url: string }
  | { action: 'wait_for'; selector: string; timeout_ms?: number }
  | { action: 'capture_console_logs'; limit?: number; clear_after_read?: boolean }
  | { action: 'capture_network_log'; limit?: number; clear_after_read?: boolean }
  | { action: 'capture_dom_snapshot'; selector?: string | null }
  | { action: 'collect_performance_entries'; entry_type?: string | null; limit?: number };

export interface BrowserConsoleEntry {
  level: string;
  args: string[];
  timestamp?: string | null;
}

export interface BrowserNetworkEvent {
  kind?: string | null;
  url?: string | null;
  method?: string | null;
  status?: number | null;
  ok?: boolean | null;
  error?: string | null;
  durationMs?: number | null;
  timestamp?: string | null;
}

export interface BrowserHarSummary {
  totalRequests: number;
  failedRequests: number;
  redirectCount: number;
  slowestRequests: string[];
}

export interface BrowserPerformanceSummary {
  summary: string;
  metrics: string[];
  longTasks: string[];
}

export interface BrowserStackFrame {
  raw: string;
  url: string;
  line: number | null;
  column: number | null;
}

interface SourceMapArtifact {
  scriptUrl: string;
  sourceMapUrl: string;
  sourceRoot: string | null;
  sources: string[];
  names: string[];
  mappings: string;
}

export interface BuiltinBrowserEvidenceSnapshot {
  targetUrl: string;
  currentUrl: string | null;
  pageTitle: string | null;
  consoleEntries: BrowserConsoleEntry[];
  networkEvents: BrowserNetworkEvent[];
  domSnapshot: Record<string, unknown> | null;
  performanceEntries: Record<string, unknown>[];
  scriptUrls: string[];
  sourceMapUrls: string[];
  resolvedSourceFiles: string[];
  stackFrames: BrowserStackFrame[];
  matchedSourceMapUrls: string[];
  originalPositionHints: string[];
  harSummary: BrowserHarSummary | null;
  performanceSummary: BrowserPerformanceSummary | null;
  captureSource: 'devtools_mcp' | 'builtin_browser';
  screenshotCaptured: boolean;
}

function normalizeToolName(name: string): string {
  return name.trim().toLowerCase();
}

function asRecord(value: unknown): JsonRecord | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as JsonRecord) : null;
}

function stringifyUnknown(value: unknown): string {
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  if (Array.isArray(value)) {
    return value
      .map((item) => stringifyUnknown(item))
      .filter((item) => item.length > 0)
      .join(' ');
  }
  if (value && typeof value === 'object') {
    const textCandidates = [
      'message',
      'text',
      'description',
      'value',
      'url',
      'stack',
      'stackTrace',
      'error',
      'exception',
      'name',
      'title',
    ];
    const record = value as JsonRecord;
    for (const key of textCandidates) {
      if (key in record) {
        const candidate = stringifyUnknown(record[key]);
        if (candidate.length > 0) return candidate;
      }
    }
  }
  return '';
}

function inferCapability(toolNames: string[]): string[] {
  const capabilities = new Set<string>();
  for (const name of toolNames.map(normalizeToolName)) {
    if (name.includes('console')) capabilities.add('console');
    if (name.includes('network')) capabilities.add('network');
    if (name.includes('snapshot') || name.includes('screenshot')) capabilities.add('snapshot');
    if (name.includes('performance') || name.includes('trace')) capabilities.add('performance');
    if (name.includes('emulate') || name.includes('resize')) capabilities.add('emulation');
    if (
      name.includes('page') ||
      name.includes('navigate') ||
      name.includes('new_page') ||
      name.includes('list_pages') ||
      name.includes('wait_for')
    ) {
      capabilities.add('navigation');
    }
    if (name.includes('evaluate') || name.includes('script')) capabilities.add('script');
  }
  return Array.from(capabilities);
}

async function executeBrowserAction(action: BrowserActionPayload): Promise<BrowserActionResult | null> {
  try {
    const result = await invoke<CommandResponse<BrowserActionResult>>('execute_browser_action', { action });
    if (!result.success || !result.data) return null;
    return result.data;
  } catch {
    return null;
  }
}

function parseJsonOutput<T>(value: string | null | undefined): T | null {
  if (!value) return null;
  try {
    return JSON.parse(value) as T;
  } catch {
    return null;
  }
}

function schemaProperties(tool: ConnectedMcpToolDetail): JsonRecord {
  const schema = asRecord(tool.input_schema);
  return asRecord(schema?.properties) ?? {};
}

function schemaRequired(tool: ConnectedMcpToolDetail): string[] {
  const schema = asRecord(tool.input_schema);
  return Array.isArray(schema?.required)
    ? schema.required.filter((item): item is string => typeof item === 'string')
    : [];
}

function findSchemaProperty(tool: ConnectedMcpToolDetail, candidates: string[]): string | null {
  const properties = schemaProperties(tool);
  for (const candidate of candidates) {
    if (candidate in properties) return candidate;
  }
  return null;
}

function buildToolArgs(
  tool: ConnectedMcpToolDetail,
  candidates: Array<{ keys: string[]; value: unknown }>,
): JsonRecord | null {
  const args: JsonRecord = {};
  for (const candidate of candidates) {
    const key = findSchemaProperty(tool, candidate.keys);
    if (key) {
      args[key] = candidate.value;
    }
  }

  const required = schemaRequired(tool);
  const missingRequired = required.filter((key) => !(key in args));
  if (missingRequired.length > 0) {
    return null;
  }

  return args;
}

function findTool(tools: ConnectedMcpToolDetail[], candidates: string[]): ConnectedMcpToolDetail | null {
  const normalizedCandidates = candidates.map(normalizeToolName);
  return (
    tools.find((tool) => normalizedCandidates.includes(normalizeToolName(tool.tool_name))) ??
    tools.find((tool) =>
      normalizedCandidates.some((candidate) => normalizeToolName(tool.tool_name).includes(candidate)),
    ) ??
    null
  );
}

async function invokeConnectedTool(
  serverId: string,
  tool: ConnectedMcpToolDetail,
  argumentsValue?: JsonRecord | null,
): Promise<unknown | null> {
  const result = await mcpApi.invokeConnectedTool(serverId, tool.tool_name, argumentsValue ?? {});
  if (!result.success || !result.data) return null;
  return result.data.value;
}

async function invokeConnectedToolIfUsable(
  serverId: string,
  tool: ConnectedMcpToolDetail | null,
  argCandidates: Array<{ keys: string[]; value: unknown }> = [],
): Promise<unknown | null> {
  if (!tool) return null;
  const args = buildToolArgs(tool, argCandidates);
  if (args == null) return null;
  return invokeConnectedTool(serverId, tool, args);
}

function collectNestedObjectArrays(value: unknown, predicate: (entry: JsonRecord) => boolean): JsonRecord[] {
  if (Array.isArray(value)) {
    const objectItems = value.filter((item): item is JsonRecord => !!asRecord(item));
    if (objectItems.length > 0 && objectItems.some(predicate)) {
      return objectItems;
    }
    return value.flatMap((item) => collectNestedObjectArrays(item, predicate));
  }

  const record = asRecord(value);
  if (!record) return [];
  return Object.values(record).flatMap((child) => collectNestedObjectArrays(child, predicate));
}

function normalizeConsoleEntries(raw: unknown): BrowserConsoleEntry[] {
  const objectEntries = collectNestedObjectArrays(
    raw,
    (entry) => 'level' in entry || 'message' in entry || 'text' in entry || 'args' in entry,
  );
  if (objectEntries.length > 0) {
    return objectEntries
      .map((entry) => {
        const argsValue = Array.isArray(entry.args)
          ? entry.args.map((item) => stringifyUnknown(item)).filter((item) => item.length > 0)
          : [];
        const message = stringifyUnknown(entry.message ?? entry.text ?? entry.value);
        const args = argsValue.length > 0 ? argsValue : message ? [message] : [];
        return {
          level: stringifyUnknown(entry.level ?? entry.type ?? entry.source ?? 'log') || 'log',
          args,
          timestamp: stringifyUnknown(entry.timestamp ?? entry.time ?? entry.created_at) || null,
        } satisfies BrowserConsoleEntry;
      })
      .filter((entry) => entry.args.length > 0)
      .slice(0, 25);
  }

  if (typeof raw === 'string') {
    return raw
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
      .slice(0, 25)
      .map((line) => ({ level: 'log', args: [line], timestamp: null }));
  }

  return [];
}

function normalizeNetworkEvents(raw: unknown): BrowserNetworkEvent[] {
  const objectEntries = collectNestedObjectArrays(
    raw,
    (entry) => 'url' in entry || 'requestUrl' in entry || 'status' in entry || 'method' in entry,
  );
  return objectEntries
    .map((entry) => {
      const statusValue = entry.status ?? entry.statusCode ?? entry.responseStatus;
      const numericStatus =
        typeof statusValue === 'number'
          ? statusValue
          : typeof statusValue === 'string' && statusValue.trim().length > 0
            ? Number(statusValue)
            : null;
      return {
        kind: stringifyUnknown(entry.kind ?? entry.type) || null,
        url: stringifyUnknown(entry.url ?? entry.requestUrl ?? entry.href) || null,
        method: stringifyUnknown(entry.method ?? entry.requestMethod) || null,
        status: Number.isFinite(numericStatus ?? NaN) ? numericStatus : null,
        ok:
          typeof entry.ok === 'boolean'
            ? entry.ok
            : Number.isFinite(numericStatus ?? NaN)
              ? (numericStatus as number) < 400
              : null,
        error: stringifyUnknown(entry.error ?? entry.failure ?? entry.failureText) || null,
        durationMs:
          typeof entry.duration === 'number'
            ? entry.duration
            : typeof entry.durationMs === 'number'
              ? entry.durationMs
              : null,
        timestamp: stringifyUnknown(entry.timestamp ?? entry.time ?? entry.startedAt) || null,
      } satisfies BrowserNetworkEvent;
    })
    .filter((entry) => !!entry.url || !!entry.error || typeof entry.status === 'number')
    .slice(0, 50);
}

function buildHarSummary(events: BrowserNetworkEvent[]): BrowserHarSummary | null {
  if (events.length === 0) return null;

  const failedRequests = events.filter(
    (event) => event.ok === false || (typeof event.status === 'number' && event.status >= 400),
  ).length;
  const redirectCount = events.filter(
    (event) => typeof event.status === 'number' && event.status >= 300 && event.status < 400,
  ).length;
  const slowestRequests = [...events]
    .filter((event) => !!event.url)
    .sort((left, right) => (right.durationMs ?? 0) - (left.durationMs ?? 0))
    .slice(0, 5)
    .map((event) => {
      const duration = typeof event.durationMs === 'number' ? `${Math.round(event.durationMs)}ms` : 'unknown duration';
      const status = typeof event.status === 'number' ? event.status : 'n/a';
      return `${event.method ?? 'GET'} ${event.url ?? '(unknown url)'} · ${status} · ${duration}`;
    });

  return {
    totalRequests: events.length,
    failedRequests,
    redirectCount,
    slowestRequests,
  };
}

function normalizeDomSnapshot(raw: unknown): Record<string, unknown> | null {
  if (!raw) return null;
  if (typeof raw === 'string') {
    return { snapshot: raw };
  }
  if (asRecord(raw)) {
    return raw as Record<string, unknown>;
  }
  return { snapshot: raw };
}

function normalizePerformanceEntries(raw: unknown): Record<string, unknown>[] {
  if (Array.isArray(raw)) {
    return raw
      .map((entry) => asRecord(entry))
      .filter((entry): entry is Record<string, unknown> => !!entry)
      .slice(0, 25);
  }
  const record = asRecord(raw);
  if (!record) return [];
  const nested = Object.values(record).find((value) => Array.isArray(value));
  return Array.isArray(nested)
    ? nested
        .map((entry) => asRecord(entry))
        .filter((entry): entry is Record<string, unknown> => !!entry)
        .slice(0, 25)
    : [];
}

function summarizePerformanceEntries(entries: Record<string, unknown>[]): BrowserPerformanceSummary | null {
  if (entries.length === 0) return null;

  const metrics: string[] = [];
  const longTasks: string[] = [];
  for (const entry of entries.slice(0, 25)) {
    const name = stringifyUnknown(entry.name ?? entry.entryType ?? entry.type ?? entry.metric) || 'entry';
    const duration =
      typeof entry.duration === 'number'
        ? entry.duration
        : typeof entry.durationMs === 'number'
          ? entry.durationMs
          : typeof entry.value === 'number'
            ? entry.value
            : null;
    const startTime =
      typeof entry.startTime === 'number' ? entry.startTime : typeof entry.ts === 'number' ? entry.ts : null;
    if (typeof duration === 'number') {
      const line = `${name} · ${Math.round(duration)}ms${typeof startTime === 'number' ? ` @ ${Math.round(startTime)}ms` : ''}`;
      metrics.push(line);
      if (duration >= 50) {
        longTasks.push(line);
      }
      continue;
    }
    const text = stringifyUnknown(entry);
    if (text) {
      metrics.push(text);
    }
  }

  const uniqueMetrics = Array.from(new Set(metrics)).slice(0, 8);
  const uniqueLongTasks = Array.from(new Set(longTasks)).slice(0, 5);
  return {
    summary:
      uniqueLongTasks.length > 0
        ? `Captured ${entries.length} performance entries with ${uniqueLongTasks.length} long tasks.`
        : `Captured ${entries.length} performance entries.`,
    metrics: uniqueMetrics,
    longTasks: uniqueLongTasks,
  };
}

function extractPageDetails(raw: unknown): { currentUrl: string | null; pageTitle: string | null } {
  const objectEntries = collectNestedObjectArrays(
    raw,
    (entry) => 'url' in entry || 'title' in entry || 'name' in entry,
  );
  const firstEntry = objectEntries[0] ?? asRecord(raw);
  if (!firstEntry) {
    return { currentUrl: null, pageTitle: null };
  }
  return {
    currentUrl: stringifyUnknown(firstEntry.url ?? firstEntry.href ?? firstEntry.currentUrl) || null,
    pageTitle: stringifyUnknown(firstEntry.title ?? firstEntry.name ?? firstEntry.label) || null,
  };
}

function normalizeUrlCandidate(value: string, baseUrl: string | null): string | null {
  const trimmed = value.trim();
  if (!trimmed || trimmed.startsWith('data:') || trimmed.startsWith('javascript:')) {
    return null;
  }
  try {
    return new URL(trimmed, baseUrl ?? undefined).toString();
  } catch {
    return null;
  }
}

function extractHtmlLikeText(domSnapshot: Record<string, unknown> | null): string {
  if (!domSnapshot) return '';
  const candidates: unknown[] = [
    domSnapshot.snapshot,
    domSnapshot.html,
    domSnapshot.outerHTML,
    domSnapshot.text,
    asRecord(domSnapshot.document)?.outerHTML,
    asRecord(domSnapshot.documentElement)?.outerHTML,
    asRecord(domSnapshot.body)?.outerHTML,
    asRecord(domSnapshot.root)?.html,
  ];
  for (const candidate of candidates) {
    if (typeof candidate === 'string' && candidate.trim().length > 0) {
      return candidate;
    }
  }
  return '';
}

function extractScriptUrlsFromSnapshot(
  domSnapshot: Record<string, unknown> | null,
  currentUrl: string | null,
): string[] {
  const html = extractHtmlLikeText(domSnapshot);
  const scriptUrls = new Set<string>();
  if (html) {
    const srcPattern = /<script[^>]+src=["']([^"']+)["']/gi;
    let match: RegExpExecArray | null = null;
    while ((match = srcPattern.exec(html)) !== null) {
      const normalized = normalizeUrlCandidate(match[1] ?? '', currentUrl);
      if (normalized) {
        scriptUrls.add(normalized);
      }
    }
  }

  if (scriptUrls.size === 0 && currentUrl) {
    scriptUrls.add(currentUrl);
  }
  return Array.from(scriptUrls).slice(0, 8);
}

async function fetchTextSafe(url: string): Promise<string | null> {
  try {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), 5000);
    const response = await fetch(url, {
      signal: controller.signal,
      credentials: 'omit',
    });
    window.clearTimeout(timeout);
    if (!response.ok) return null;
    return await response.text();
  } catch {
    return null;
  }
}

function extractSourceMapUrlFromScript(scriptBody: string, scriptUrl: string): string | null {
  const patterns = [/\/\/[#@]\s*sourceMappingURL=([^\s]+)/g, /\/\*[#@]\s*sourceMappingURL=([^*]+)\*\//g];
  for (const pattern of patterns) {
    const matches = Array.from(scriptBody.matchAll(pattern));
    const lastMatch = matches[matches.length - 1];
    const raw = lastMatch?.[1]?.trim();
    if (!raw) continue;
    const normalized = normalizeUrlCandidate(raw, scriptUrl);
    if (normalized) return normalized;
  }

  return normalizeUrlCandidate(`${scriptUrl}.map`, scriptUrl);
}

async function resolveSourceMapArtifacts(
  scriptUrls: string[],
): Promise<{ sourceMapUrls: string[]; resolvedSourceFiles: string[]; artifacts: SourceMapArtifact[] }> {
  const sourceMapUrls = new Set<string>();
  const resolvedSourceFiles = new Set<string>();
  const artifacts: SourceMapArtifact[] = [];

  for (const scriptUrl of scriptUrls.slice(0, 5)) {
    const scriptBody = await fetchTextSafe(scriptUrl);
    if (!scriptBody) continue;
    const sourceMapUrl = extractSourceMapUrlFromScript(scriptBody, scriptUrl);
    if (!sourceMapUrl) continue;
    sourceMapUrls.add(sourceMapUrl);

    const mapBody = await fetchTextSafe(sourceMapUrl);
    if (!mapBody) continue;
    try {
      const parsed = JSON.parse(mapBody) as JsonRecord;
      const sources = Array.isArray(parsed.sources) ? parsed.sources : [];
      const sourceRoot = typeof parsed.sourceRoot === 'string' ? parsed.sourceRoot : null;
      for (const source of sources) {
        if (typeof source !== 'string' || source.trim().length === 0) continue;
        const normalized = normalizeUrlCandidate(source, sourceMapUrl) ?? source.trim();
        resolvedSourceFiles.add(normalized);
      }
      artifacts.push({
        scriptUrl,
        sourceMapUrl,
        sourceRoot,
        sources: sources.filter((source): source is string => typeof source === 'string'),
        names: Array.isArray(parsed.names)
          ? parsed.names.filter((name): name is string => typeof name === 'string')
          : [],
        mappings: typeof parsed.mappings === 'string' ? parsed.mappings : '',
      });
    } catch {
      continue;
    }
  }

  return {
    sourceMapUrls: Array.from(sourceMapUrls).slice(0, 6),
    resolvedSourceFiles: Array.from(resolvedSourceFiles).slice(0, 12),
    artifacts,
  };
}

const BASE64_VLQ_CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

function decodeBase64VlqSegment(segment: string): number[] {
  const values: number[] = [];
  let value = 0;
  let shift = 0;
  for (const char of segment) {
    const digit = BASE64_VLQ_CHARS.indexOf(char);
    if (digit < 0) continue;
    const continuation = (digit & 32) !== 0;
    const chunk = digit & 31;
    value += chunk << shift;
    if (continuation) {
      shift += 5;
      continue;
    }
    const negative = (value & 1) === 1;
    const decoded = value >> 1;
    values.push(negative ? -decoded : decoded);
    value = 0;
    shift = 0;
  }
  return values;
}

function resolveOriginalSourcePath(source: string, sourceRoot: string | null, sourceMapUrl: string): string {
  const combined = sourceRoot ? `${sourceRoot.replace(/\/$/, '')}/${source.replace(/^\//, '')}` : source;
  return normalizeUrlCandidate(combined, sourceMapUrl) ?? combined;
}

function findOriginalPositionForFrame(
  frame: BrowserStackFrame,
  artifact: SourceMapArtifact,
): { source: string; line: number | null; column: number | null; name: string | null } | null {
  if (!frame.line || !artifact.mappings) return null;
  if (!(frame.url.includes(artifact.scriptUrl) || artifact.scriptUrl.includes(frame.url))) {
    return null;
  }

  let generatedLine = 0;
  let previousGeneratedColumn = 0;
  let previousSourceIndex = 0;
  let previousOriginalLine = 0;
  let previousOriginalColumn = 0;
  let previousNameIndex = 0;
  let bestMatch: {
    source: string;
    line: number | null;
    column: number | null;
    name: string | null;
    generatedColumn: number;
  } | null = null;

  const targetGeneratedLine = frame.line - 1;
  const targetGeneratedColumn = Math.max((frame.column ?? 1) - 1, 0);
  const mappingLines = artifact.mappings.split(';');
  for (const lineSegments of mappingLines) {
    if (generatedLine > targetGeneratedLine) break;
    previousGeneratedColumn = 0;
    if (generatedLine === targetGeneratedLine) {
      for (const segment of lineSegments.split(',')) {
        if (!segment) continue;
        const decoded = decodeBase64VlqSegment(segment);
        if (decoded.length < 4) continue;
        previousGeneratedColumn += decoded[0] ?? 0;
        previousSourceIndex += decoded[1] ?? 0;
        previousOriginalLine += decoded[2] ?? 0;
        previousOriginalColumn += decoded[3] ?? 0;
        if (decoded.length >= 5) {
          previousNameIndex += decoded[4] ?? 0;
        }
        if (previousGeneratedColumn > targetGeneratedColumn && bestMatch) {
          break;
        }
        if (previousSourceIndex < 0 || previousSourceIndex >= artifact.sources.length) continue;
        bestMatch = {
          source: resolveOriginalSourcePath(
            artifact.sources[previousSourceIndex] ?? '',
            artifact.sourceRoot,
            artifact.sourceMapUrl,
          ),
          line: previousOriginalLine + 1,
          column: previousOriginalColumn,
          name: artifact.names[previousNameIndex] ?? null,
          generatedColumn: previousGeneratedColumn,
        };
      }
      break;
    }

    for (const segment of lineSegments.split(',')) {
      if (!segment) continue;
      const decoded = decodeBase64VlqSegment(segment);
      if (decoded.length < 4) continue;
      previousGeneratedColumn += decoded[0] ?? 0;
      previousSourceIndex += decoded[1] ?? 0;
      previousOriginalLine += decoded[2] ?? 0;
      previousOriginalColumn += decoded[3] ?? 0;
      if (decoded.length >= 5) {
        previousNameIndex += decoded[4] ?? 0;
      }
    }
    generatedLine += 1;
  }

  return bestMatch
    ? {
        source: bestMatch.source,
        line: bestMatch.line,
        column: bestMatch.column,
        name: bestMatch.name,
      }
    : null;
}

function buildOriginalPositionHints(stackFrames: BrowserStackFrame[], artifacts: SourceMapArtifact[]): string[] {
  const hints = new Set<string>();
  for (const frame of stackFrames) {
    for (const artifact of artifacts) {
      const original = findOriginalPositionForFrame(frame, artifact);
      if (!original) continue;
      const location = `${original.source}${original.line ? `:${original.line}` : ''}${typeof original.column === 'number' ? `:${original.column}` : ''}`;
      const detail = original.name ? `${frame.raw} -> ${location} (${original.name})` : `${frame.raw} -> ${location}`;
      hints.add(detail);
      break;
    }
  }
  return Array.from(hints).slice(0, 8);
}

function extractStackFrames(consoleEntries: BrowserConsoleEntry[], scriptUrls: string[]): BrowserStackFrame[] {
  const frames: BrowserStackFrame[] = [];
  const seen = new Set<string>();
  const patterns = [/(https?:\/\/[^\s)]+):(\d+):(\d+)/g, /\b([\w./-]+\.(?:js|jsx|ts|tsx)):(\d+):(\d+)/g];

  for (const entry of consoleEntries) {
    const message = entry.args.join(' ');
    for (const pattern of patterns) {
      let match: RegExpExecArray | null = null;
      while ((match = pattern.exec(message)) !== null) {
        const rawUrl = match[1] ?? '';
        const url = normalizeUrlCandidate(rawUrl, scriptUrls[0] ?? null) ?? rawUrl;
        const key = `${url}:${match[2] ?? ''}:${match[3] ?? ''}`;
        if (seen.has(key)) continue;
        seen.add(key);
        frames.push({
          raw: match[0] ?? key,
          url,
          line: Number(match[2] ?? '') || null,
          column: Number(match[3] ?? '') || null,
        });
      }
    }
  }

  return frames.slice(0, 8);
}

function matchSourceMapsToStackFrames(
  stackFrames: BrowserStackFrame[],
  sourceMapUrls: string[],
  scriptUrls: string[],
): string[] {
  const matches = new Set<string>();
  for (const frame of stackFrames) {
    for (const sourceMapUrl of sourceMapUrls) {
      if (sourceMapUrl.includes(frame.url) || frame.url.includes(sourceMapUrl.replace(/\.map$/, ''))) {
        matches.add(sourceMapUrl);
      }
    }
    for (const scriptUrl of scriptUrls) {
      if (frame.url.includes(scriptUrl) || scriptUrl.includes(frame.url)) {
        const sourceMapUrl = sourceMapUrls.find((candidate) => candidate.includes(scriptUrl));
        if (sourceMapUrl) {
          matches.add(sourceMapUrl);
        }
      }
    }
  }
  return Array.from(matches).slice(0, 6);
}

async function collectDevtoolsBrowserEvidence(
  targetUrl: string,
  serverId: string,
): Promise<BuiltinBrowserEvidenceSnapshot | null> {
  const toolsResponse = await mcpApi.getConnectedServerTools(serverId);
  const tools = toolsResponse.success && toolsResponse.data ? toolsResponse.data : [];
  if (tools.length === 0) return null;

  const navigateTool = findTool(tools, ['new_page', 'navigate_page']);
  if (navigateTool) {
    const args = buildToolArgs(navigateTool, [
      { keys: ['url', 'pageUrl'], value: targetUrl },
      { keys: ['timeout', 'timeout_ms'], value: 5000 },
    ]);
    if (args) {
      await invokeConnectedTool(serverId, navigateTool, args);
    }
  }

  const listPagesTool = findTool(tools, ['list_pages']);
  const consoleTool = findTool(tools, ['list_console_messages', 'get_console_messages']);
  const networkTool = findTool(tools, ['list_network_requests', 'get_network_requests']);
  const snapshotTool = findTool(tools, ['take_snapshot', 'capture_snapshot']);
  const screenshotTool = findTool(tools, ['take_screenshot', 'capture_screenshot']);
  const performanceTool = findTool(tools, ['performance_analyze_insight', 'list_performance_entries']);

  const [pageRaw, consoleRaw, networkRaw, snapshotRaw, screenshotRaw, performanceRaw] = await Promise.all([
    invokeConnectedToolIfUsable(serverId, listPagesTool),
    invokeConnectedToolIfUsable(serverId, consoleTool),
    invokeConnectedToolIfUsable(serverId, networkTool),
    invokeConnectedToolIfUsable(serverId, snapshotTool, [{ keys: ['verbose'], value: false }]),
    invokeConnectedToolIfUsable(serverId, screenshotTool, [{ keys: ['fullPage', 'full_page'], value: true }]),
    invokeConnectedToolIfUsable(serverId, performanceTool),
  ]);

  const pageDetails = extractPageDetails(pageRaw);
  const consoleEntries = normalizeConsoleEntries(consoleRaw);
  const networkEvents = normalizeNetworkEvents(networkRaw);
  const domSnapshot = normalizeDomSnapshot(snapshotRaw);
  const performanceEntries = normalizePerformanceEntries(performanceRaw);
  const scriptUrls = extractScriptUrlsFromSnapshot(domSnapshot, pageDetails.currentUrl || targetUrl);
  const { sourceMapUrls, resolvedSourceFiles, artifacts } = await resolveSourceMapArtifacts(scriptUrls);
  const stackFrames = extractStackFrames(consoleEntries, scriptUrls);
  const matchedSourceMapUrls = matchSourceMapsToStackFrames(stackFrames, sourceMapUrls, scriptUrls);
  const originalPositionHints = buildOriginalPositionHints(stackFrames, artifacts);
  const harSummary = buildHarSummary(networkEvents);
  const performanceSummary = summarizePerformanceEntries(performanceEntries);

  if (
    !pageDetails.currentUrl &&
    consoleEntries.length === 0 &&
    networkEvents.length === 0 &&
    !domSnapshot &&
    performanceEntries.length === 0 &&
    !screenshotRaw &&
    scriptUrls.length === 0
  ) {
    return null;
  }

  return {
    targetUrl,
    currentUrl: pageDetails.currentUrl || targetUrl,
    pageTitle: pageDetails.pageTitle,
    consoleEntries,
    networkEvents,
    domSnapshot,
    performanceEntries,
    scriptUrls,
    sourceMapUrls,
    resolvedSourceFiles,
    stackFrames,
    matchedSourceMapUrls,
    originalPositionHints,
    harSummary,
    performanceSummary,
    captureSource: 'devtools_mcp',
    screenshotCaptured: screenshotRaw != null,
  };
}

export async function getDebugBrowserBridgeStatus(): Promise<DebugBrowserBridgeStatus> {
  const [browserStatusResponse, serversResponse, connectedResponse] = await Promise.all([
    invoke<CommandResponse<BrowserAvailability>>('get_browser_status').catch(() => null),
    mcpApi.listServers(),
    mcpApi.listConnectedServers(),
  ]);

  const browserStatus = browserStatusResponse?.success ? browserStatusResponse.data : null;
  const builtinBrowserAvailable = !!browserStatus?.feature_compiled && !!browserStatus?.browser_detected;

  const servers = serversResponse.success && serversResponse.data ? serversResponse.data : [];
  const connected = connectedResponse.success && connectedResponse.data ? connectedResponse.data : [];
  const serverById = new Map(servers.map((server) => [server.id, server]));

  const installedDevtoolsServer =
    servers.find((server) => server.catalog_item_id === 'chrome-devtools-mcp') ||
    servers.find((server) => normalizeToolName(server.name).includes('chrome devtools'));
  const connectedDevtoolsServer =
    connected.find((server) => serverById.get(server.server_id)?.catalog_item_id === 'chrome-devtools-mcp') ||
    connected.find((server) => normalizeToolName(server.server_name).includes('chrome devtools'));

  let connectedToolNames: string[] = [];
  if (connectedDevtoolsServer) {
    const toolsResponse = await mcpApi.getConnectedServerTools(connectedDevtoolsServer.server_id);
    if (toolsResponse.success && toolsResponse.data) {
      connectedToolNames = toolsResponse.data.map((tool) => tool.tool_name);
    }
  }

  const devtoolsConnected = !!connectedDevtoolsServer;
  const devtoolsCatalogInstalled = !!installedDevtoolsServer;
  const capabilities = inferCapability(connectedToolNames);

  const notes: string[] = [];
  if (devtoolsConnected) {
    notes.push('bridgeNotes.devtoolsConnected');
  } else if (devtoolsCatalogInstalled) {
    notes.push('bridgeNotes.installedNotConnected');
  } else {
    notes.push('bridgeNotes.catalogAvailable');
  }
  if (builtinBrowserAvailable) {
    notes.push('bridgeNotes.builtinFallback');
  }

  return {
    kind: devtoolsConnected ? 'devtools_mcp' : builtinBrowserAvailable ? 'builtin_browser' : 'unavailable',
    builtinBrowserAvailable,
    devtoolsCatalogInstalled,
    devtoolsConnected,
    serverId: connectedDevtoolsServer?.server_id ?? installedDevtoolsServer?.id ?? null,
    serverName: connectedDevtoolsServer?.server_name ?? installedDevtoolsServer?.name ?? null,
    capabilities,
    connectedToolNames,
    recommendedCatalogItemId: 'chrome-devtools-mcp',
    notes,
  };
}

export async function collectBuiltinBrowserEvidence(targetUrl: string): Promise<BuiltinBrowserEvidenceSnapshot | null> {
  const openPage = await executeBrowserAction({ action: 'open_page', url: targetUrl });
  if (!openPage?.success) return null;

  await executeBrowserAction({ action: 'wait_for', selector: 'body', timeout_ms: 5000 });

  const [consoleResult, networkResult, domResult, performanceResult] = await Promise.all([
    executeBrowserAction({ action: 'capture_console_logs', limit: 25, clear_after_read: false }),
    executeBrowserAction({ action: 'capture_network_log', limit: 25, clear_after_read: false }),
    executeBrowserAction({ action: 'capture_dom_snapshot', selector: null }),
    executeBrowserAction({ action: 'collect_performance_entries', limit: 10 }),
  ]);

  const consolePayload = parseJsonOutput<{ logs?: BrowserConsoleEntry[] }>(consoleResult?.output);
  const networkPayload = parseJsonOutput<{ events?: BrowserNetworkEvent[] }>(networkResult?.output);
  const domPayload = parseJsonOutput<Record<string, unknown>>(domResult?.output);
  const performancePayload = parseJsonOutput<Record<string, unknown>[]>(performanceResult?.output);
  const currentUrl = openPage.current_url ?? consoleResult?.current_url ?? targetUrl;
  const scriptUrls = extractScriptUrlsFromSnapshot(domPayload, currentUrl);
  const { sourceMapUrls, resolvedSourceFiles, artifacts } = await resolveSourceMapArtifacts(scriptUrls);
  const consoleEntries = consolePayload?.logs ?? [];
  const networkEvents = networkPayload?.events ?? [];
  const performanceEntries = performancePayload ?? [];
  const stackFrames = extractStackFrames(consoleEntries, scriptUrls);
  const matchedSourceMapUrls = matchSourceMapsToStackFrames(stackFrames, sourceMapUrls, scriptUrls);
  const originalPositionHints = buildOriginalPositionHints(stackFrames, artifacts);
  const harSummary = buildHarSummary(networkEvents);
  const performanceSummary = summarizePerformanceEntries(performanceEntries);

  return {
    targetUrl,
    currentUrl: currentUrl ?? null,
    pageTitle: openPage.page_title ?? consoleResult?.page_title ?? null,
    consoleEntries,
    networkEvents,
    domSnapshot: domPayload,
    performanceEntries,
    scriptUrls,
    sourceMapUrls,
    resolvedSourceFiles,
    stackFrames,
    matchedSourceMapUrls,
    originalPositionHints,
    harSummary,
    performanceSummary,
    captureSource: 'builtin_browser',
    screenshotCaptured: false,
  };
}

export async function collectPreferredBrowserEvidence(
  targetUrl: string,
  bridgeStatus?: DebugBrowserBridgeStatus,
): Promise<BuiltinBrowserEvidenceSnapshot | null> {
  const status = bridgeStatus ?? (await getDebugBrowserBridgeStatus());

  if (status.devtoolsConnected && status.serverId) {
    const devtoolsEvidence = await collectDevtoolsBrowserEvidence(targetUrl, status.serverId);
    if (devtoolsEvidence) {
      return devtoolsEvidence;
    }
  }

  if (status.builtinBrowserAvailable) {
    return collectBuiltinBrowserEvidence(targetUrl);
  }

  return null;
}
