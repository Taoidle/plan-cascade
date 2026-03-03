/**
 * LSP Type Definitions
 *
 * TypeScript types for LSP server detection and enrichment management.
 * Mirrors the Rust types in commands/lsp.rs.
 */

/** Supported LSP languages */
export type LspLanguage = 'rust' | 'python' | 'go' | 'typescript' | 'java';

/** Per-language server detection status */
export interface LspServerStatus {
  language: LspLanguage;
  server_name: string;
  detected: boolean;
  binary_path: string | null;
  version: string | null;
  detected_at: string | null;
  install_hint: string;
}

/** Persisted LSP preferences (backend source of truth). */
export interface LspPreferences {
  autoEnrich: boolean;
  incrementalDebounceMs: number;
}

/** Enrichment pass results */
export interface EnrichmentReport {
  languages_enriched: string[];
  symbols_enriched: number;
  references_found: number;
  duration_ms: number;
}

/** Language display metadata (for the UI) */
export interface LspLanguageInfo {
  id: LspLanguage;
  displayName: string;
  serverName: string;
}

/** All supported languages with display metadata */
export const LSP_LANGUAGES: LspLanguageInfo[] = [
  { id: 'rust', displayName: 'Rust', serverName: 'rust-analyzer' },
  { id: 'python', displayName: 'Python', serverName: 'pyright' },
  { id: 'go', displayName: 'Go', serverName: 'gopls' },
  { id: 'typescript', displayName: 'TypeScript/JS', serverName: 'vtsls' },
  { id: 'java', displayName: 'Java', serverName: 'jdtls' },
];
