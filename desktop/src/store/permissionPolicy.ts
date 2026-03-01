/**
 * Permission Policy Store
 *
 * Manages runtime policy configuration (Policy v2) with local persistence.
 * Changes are applied to backend immediately for real-time effect.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { getPermissionPolicyConfig, setPermissionPolicyConfig } from '../lib/permissionPolicyApi';

interface PermissionPolicyState {
  networkDomainAllowlist: string[];
  builtinNetworkDomainAllowlist: string[];
  builtinNetworkDomainAllowlistVersion: string;
  builtinNetworkDomainAllowlistAvailableVersions: string[];
  loading: boolean;
  saving: boolean;
  initialized: boolean;
  error: string | null;

  initializePolicy: () => Promise<void>;
  fetchPolicyConfig: () => Promise<void>;
  setNetworkDomainAllowlist: (domains: string[]) => Promise<boolean>;
  clearError: () => void;
}

function isTauriAvailable(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function normalizeDomainCandidate(raw: string): string | null {
  let input = raw.trim().toLowerCase();
  if (!input) return null;

  if (input.includes('://')) {
    try {
      const parsed = new URL(input);
      input = parsed.hostname;
    } catch {
      return null;
    }
  } else {
    input = input
      .replace(/^[a-z]+:\/\//i, '')
      .split('/')[0]
      .split('?')[0]
      .split('#')[0];
  }

  input = input.trim().replace(/^\.+/, '').replace(/\.+$/, '');
  if (!input) return null;

  // Strip :port for non-IPv6 host format.
  if (input.includes(':') && input.split(':').length === 2) {
    const [host, port] = input.split(':');
    if (port && /^\d+$/.test(port)) {
      input = host;
    }
  }

  // Keep aligned with backend host normalization (alnum, dot, dash, underscore).
  if (!/^[a-z0-9._-]+$/.test(input)) return null;
  return input;
}

function normalizeDomainList(domains: string[]): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const domain of domains) {
    const value = normalizeDomainCandidate(domain);
    if (!value || seen.has(value)) continue;
    seen.add(value);
    normalized.push(value);
  }
  return normalized;
}

export const usePermissionPolicyStore = create<PermissionPolicyState>()(
  persist(
    (set, get) => ({
      networkDomainAllowlist: [],
      builtinNetworkDomainAllowlist: [],
      builtinNetworkDomainAllowlistVersion: '',
      builtinNetworkDomainAllowlistAvailableVersions: [],
      loading: false,
      saving: false,
      initialized: false,
      error: null,

      initializePolicy: async () => {
        if (get().initialized) return;

        if (!isTauriAvailable()) {
          set({ initialized: true });
          return;
        }

        set({ loading: true, error: null });
        try {
          const persisted = normalizeDomainList(get().networkDomainAllowlist);

          // Apply persisted local policy to backend runtime for startup consistency.
          const applyResult = await setPermissionPolicyConfig({
            network_domain_allowlist: persisted,
          });
          if (!applyResult.success) {
            throw new Error(applyResult.error ?? 'Failed to apply persisted permission policy');
          }

          const latest = await getPermissionPolicyConfig();
          if (!latest.success || !latest.data) {
            throw new Error(latest.error ?? 'Failed to fetch permission policy');
          }

          set({
            networkDomainAllowlist: normalizeDomainList(latest.data.network_domain_allowlist),
            builtinNetworkDomainAllowlist: normalizeDomainList(latest.data.builtin_network_domain_allowlist ?? []),
            builtinNetworkDomainAllowlistVersion: latest.data.builtin_network_domain_allowlist_version ?? '',
            builtinNetworkDomainAllowlistAvailableVersions:
              latest.data.builtin_network_domain_allowlist_available_versions ?? [],
            loading: false,
            initialized: true,
            error: null,
          });
        } catch (error) {
          set({
            loading: false,
            initialized: true,
            error: error instanceof Error ? error.message : String(error),
          });
        }
      },

      fetchPolicyConfig: async () => {
        if (!isTauriAvailable()) return;

        set({ loading: true, error: null });
        try {
          const response = await getPermissionPolicyConfig();
          if (!response.success || !response.data) {
            throw new Error(response.error ?? 'Failed to fetch permission policy');
          }
          set({
            networkDomainAllowlist: normalizeDomainList(response.data.network_domain_allowlist),
            builtinNetworkDomainAllowlist: normalizeDomainList(response.data.builtin_network_domain_allowlist ?? []),
            builtinNetworkDomainAllowlistVersion: response.data.builtin_network_domain_allowlist_version ?? '',
            builtinNetworkDomainAllowlistAvailableVersions:
              response.data.builtin_network_domain_allowlist_available_versions ?? [],
            loading: false,
            initialized: true,
            error: null,
          });
        } catch (error) {
          set({
            loading: false,
            error: error instanceof Error ? error.message : String(error),
          });
        }
      },

      setNetworkDomainAllowlist: async (domains) => {
        if (!isTauriAvailable()) {
          set({ networkDomainAllowlist: normalizeDomainList(domains) });
          return true;
        }

        const previous = get().networkDomainAllowlist;
        const normalized = normalizeDomainList(domains);

        set({
          networkDomainAllowlist: normalized,
          saving: true,
          error: null,
        });

        try {
          const response = await setPermissionPolicyConfig({
            network_domain_allowlist: normalized,
          });
          if (!response.success) {
            throw new Error(response.error ?? 'Failed to save permission policy');
          }

          set({
            saving: false,
            initialized: true,
            error: null,
          });
          return true;
        } catch (error) {
          set({
            networkDomainAllowlist: previous,
            saving: false,
            error: error instanceof Error ? error.message : String(error),
          });
          return false;
        }
      },

      clearError: () => set({ error: null }),
    }),
    {
      name: 'plan-cascade-permission-policy',
      version: 1,
      partialize: (state) => ({
        networkDomainAllowlist: state.networkDomainAllowlist,
      }),
    },
  ),
);
