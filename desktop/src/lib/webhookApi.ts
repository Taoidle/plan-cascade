/**
 * Webhook API (IPC Wrappers)
 *
 * Type-safe wrappers for the Tauri webhook IPC commands defined in
 * `src-tauri/src/commands/webhook.rs`.
 */

import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from './tauri';

// ---------------------------------------------------------------------------
// Types (mirror Rust types with snake_case field names)
// ---------------------------------------------------------------------------

export type WebhookChannelType = 'Slack' | 'Feishu' | 'Telegram' | 'Discord' | 'Custom';

export type WebhookScope =
  | 'Global'
  | { Sessions: string[] };

export type WebhookEventType =
  | 'TaskComplete'
  | 'TaskFailed'
  | 'TaskCancelled'
  | 'StoryComplete'
  | 'PrdComplete'
  | 'ProgressMilestone';

export type DeliveryStatus = 'Pending' | 'Success' | 'Failed' | 'Retrying';

export interface TokenUsageSummary {
  input_tokens?: number;
  output_tokens?: number;
}

export interface WebhookChannelConfig {
  id: string;
  name: string;
  channel_type: WebhookChannelType;
  enabled: boolean;
  url: string;
  scope: WebhookScope;
  events: WebhookEventType[];
  template?: string;
  created_at: string;
  updated_at: string;
}

export interface WebhookPayload {
  event_type: WebhookEventType;
  session_id?: string;
  session_name?: string;
  project_path?: string;
  summary: string;
  details?: unknown;
  timestamp: string;
  duration_ms?: number;
  token_usage?: TokenUsageSummary;
}

export interface WebhookDelivery {
  id: string;
  channel_id: string;
  event_type: WebhookEventType;
  payload: WebhookPayload;
  status: DeliveryStatus;
  status_code?: number;
  response_body?: string;
  attempts: number;
  last_attempt_at: string;
  created_at: string;
}

export interface WebhookTestResult {
  success: boolean;
  latency_ms?: number;
  error?: string;
}

export interface CreateWebhookRequest {
  name: string;
  channel_type: WebhookChannelType;
  url: string;
  secret?: string;
  scope: WebhookScope;
  events: WebhookEventType[];
  template?: string;
}

export interface UpdateWebhookRequest {
  name?: string;
  url?: string;
  secret?: string;
  scope?: WebhookScope;
  events?: WebhookEventType[];
  template?: string;
  enabled?: boolean;
}

// ---------------------------------------------------------------------------
// IPC Wrappers
// ---------------------------------------------------------------------------

/**
 * List all configured webhook channels.
 */
export async function listWebhookChannels(): Promise<CommandResponse<WebhookChannelConfig[]>> {
  try {
    return await invoke<CommandResponse<WebhookChannelConfig[]>>('list_webhook_channels');
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Create a new webhook channel.
 */
export async function createWebhookChannel(
  request: CreateWebhookRequest,
): Promise<CommandResponse<WebhookChannelConfig>> {
  try {
    return await invoke<CommandResponse<WebhookChannelConfig>>('create_webhook_channel', {
      request,
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
 * Update an existing webhook channel.
 */
export async function updateWebhookChannel(
  id: string,
  request: UpdateWebhookRequest,
): Promise<CommandResponse<WebhookChannelConfig>> {
  try {
    return await invoke<CommandResponse<WebhookChannelConfig>>('update_webhook_channel', {
      id,
      request,
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
 * Delete a webhook channel.
 */
export async function deleteWebhookChannel(
  id: string,
): Promise<CommandResponse<null>> {
  try {
    return await invoke<CommandResponse<null>>('delete_webhook_channel', { id });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Test a webhook channel by sending a test notification.
 */
export async function testWebhookChannel(
  id: string,
): Promise<CommandResponse<WebhookTestResult>> {
  try {
    return await invoke<CommandResponse<WebhookTestResult>>('test_webhook_channel', { id });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Get delivery history with optional channel_id filter and pagination.
 */
export async function getWebhookDeliveries(
  channelId?: string,
  limit?: number,
  offset?: number,
): Promise<CommandResponse<WebhookDelivery[]>> {
  try {
    return await invoke<CommandResponse<WebhookDelivery[]>>('get_webhook_deliveries', {
      channel_id: channelId,
      limit,
      offset,
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
 * Retry a failed delivery.
 */
export async function retryWebhookDelivery(
  deliveryId: string,
): Promise<CommandResponse<WebhookDelivery>> {
  try {
    return await invoke<CommandResponse<WebhookDelivery>>('retry_webhook_delivery', {
      delivery_id: deliveryId,
    });
  } catch (error) {
    return {
      success: false,
      data: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
