/**
 * LSP API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri LSP commands defined in
 * `src-tauri/src/commands/lsp.rs`. Each function follows the project
 * IPC pattern: `invoke<CommandResponse<T>>('command_name', { params })`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';
import type { LspServerStatus, EnrichmentReport } from '../types/lsp';

// ---------------------------------------------------------------------------
// detect_lsp_servers
// ---------------------------------------------------------------------------

/**
 * Run server detection for all 5 supported languages.
 *
 * Checks PATH and known fallback locations for each language server binary.
 * Results are cached in the lsp_servers table.
 */
export async function detectLspServers(): Promise<CommandResponse<LspServerStatus[]>> {
  try {
    return await invoke<CommandResponse<LspServerStatus[]>>('detect_lsp_servers');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// get_lsp_status
// ---------------------------------------------------------------------------

/**
 * Get the current per-language server status.
 *
 * Returns cached detection results or runs detection if not yet cached.
 */
export async function getLspStatus(): Promise<CommandResponse<LspServerStatus[]>> {
  try {
    return await invoke<CommandResponse<LspServerStatus[]>>('get_lsp_status');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// trigger_lsp_enrichment
// ---------------------------------------------------------------------------

/**
 * Manually trigger an LSP enrichment pass for a project.
 *
 * Creates LSP clients for detected servers, queries hover/references for
 * each indexed symbol, and stores the enriched data.
 */
export async function triggerLspEnrichment(projectPath: string): Promise<CommandResponse<EnrichmentReport>> {
  try {
    return await invoke<CommandResponse<EnrichmentReport>>('trigger_lsp_enrichment', {
      projectPath,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

// ---------------------------------------------------------------------------
// get_enrichment_report
// ---------------------------------------------------------------------------

/**
 * Get the results of the last enrichment pass.
 */
export async function getEnrichmentReport(): Promise<CommandResponse<EnrichmentReport | null>> {
  try {
    return await invoke<CommandResponse<EnrichmentReport | null>>('get_enrichment_report');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
