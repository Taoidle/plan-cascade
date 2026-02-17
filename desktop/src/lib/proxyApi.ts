/**
 * Proxy API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri proxy IPC commands defined in
 * `src-tauri/src/commands/proxy.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ProxyProtocol = 'http' | 'https' | 'socks5';

export type ProxyStrategy = 'use_global' | 'no_proxy' | 'custom';

export interface ProxyConfig {
  protocol: ProxyProtocol;
  host: string;
  port: number;
  username?: string;
  password?: string;
}

export interface ProxySettingsResponse {
  global: ProxyConfig | null;
  provider_strategies: Record<string, ProxyStrategy>;
  provider_configs: Record<string, ProxyConfig>;
}

export interface SetProxyConfigRequest {
  proxy: ProxyConfig | null;
  password?: string;
}

export interface SetProviderProxyRequest {
  provider: string;
  strategy: ProxyStrategy;
  custom_proxy?: ProxyConfig;
  custom_password?: string;
}

export interface TestProxyRequest {
  proxy: ProxyConfig;
  password?: string;
  test_url?: string;
}

export interface ProxyTestResult {
  success: boolean;
  latency_ms?: number;
  error?: string;
}

// ---------------------------------------------------------------------------
// IPC Wrappers
// ---------------------------------------------------------------------------

/**
 * Retrieve the full proxy configuration (global + per-provider strategies).
 */
export async function getProxyConfig(): Promise<CommandResponse<ProxySettingsResponse>> {
  try {
    return await invoke<CommandResponse<ProxySettingsResponse>>('get_proxy_config');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Set or clear the global proxy configuration.
 */
export async function setProxyConfig(
  request: SetProxyConfigRequest,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('set_proxy_config', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Get the proxy strategy for a specific provider.
 */
export async function getProviderProxyStrategy(
  provider: string,
): Promise<CommandResponse<ProxyStrategy>> {
  try {
    return await invoke<CommandResponse<ProxyStrategy>>('get_provider_proxy_strategy', {
      provider,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Set the proxy strategy (and optional custom config) for a specific provider.
 */
export async function setProviderProxyStrategy(
  request: SetProviderProxyRequest,
): Promise<CommandResponse<boolean>> {
  try {
    return await invoke<CommandResponse<boolean>>('set_provider_proxy_strategy', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Test proxy connectivity.
 */
export async function testProxy(
  request: TestProxyRequest,
): Promise<CommandResponse<ProxyTestResult>> {
  try {
    return await invoke<CommandResponse<ProxyTestResult>>('test_proxy', { request });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
