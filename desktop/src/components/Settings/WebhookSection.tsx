/**
 * WebhookSection Component
 *
 * Production-grade settings panel for webhook notification channels.
 */

import { useEffect, useMemo, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useWebhookStore } from '../../store/webhook';
import type {
  CreateWebhookRequest,
  UpdateWebhookRequest,
  WebhookChannelConfig,
  WebhookChannelType,
  WebhookEventType,
  WebhookScope,
} from '../../lib/webhookApi';

const CHANNEL_TYPES: { value: WebhookChannelType; label: string }[] = [
  { value: 'Slack', label: 'Slack' },
  { value: 'Feishu', label: 'Feishu / Lark' },
  { value: 'Telegram', label: 'Telegram' },
  { value: 'ServerChan', label: 'ServerChan' },
  { value: 'Discord', label: 'Discord' },
  { value: 'Custom', label: 'Custom HTTP' },
];

const EVENT_TYPES: { value: WebhookEventType; labelKey: string }[] = [
  { value: 'TaskComplete', labelKey: 'webhook.events.taskComplete' },
  { value: 'TaskFailed', labelKey: 'webhook.events.taskFailed' },
  { value: 'TaskCancelled', labelKey: 'webhook.events.taskCancelled' },
  { value: 'StoryComplete', labelKey: 'webhook.events.storyComplete' },
  { value: 'PrdComplete', labelKey: 'webhook.events.prdComplete' },
  { value: 'ProgressMilestone', labelKey: 'webhook.events.progressMilestone' },
];

const MAX_NAME_LEN = 80;
const MAX_TEMPLATE_LEN = 4000;
const MAX_SCOPE_SESSIONS = 100;
const MAX_SESSION_ID_LEN = 128;
const PAGE_SIZE = 20;

type FormErrors = {
  name?: string;
  url?: string;
  secret?: string;
  events?: string;
  scopeSessions?: string;
  template?: string;
};

function isTelegramChatId(target: string): boolean {
  const trimmed = target.trim();
  if (!trimmed) {
    return false;
  }
  if (trimmed.startsWith('@')) {
    const name = trimmed.slice(1);
    return !!name && /^[a-zA-Z0-9_]+$/.test(name);
  }
  return /^-?\d+$/.test(trimmed);
}

function isPrivateHost(hostname: string): boolean {
  const host = hostname.toLowerCase();
  if (
    host === 'localhost' ||
    host === '::1' ||
    host === '::' ||
    host === '0.0.0.0' ||
    host.endsWith('.local') ||
    host.endsWith('.internal')
  ) {
    return true;
  }
  if (/^127\./.test(host)) {
    return true;
  }

  const ipv4 = host.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)$/);
  if (ipv4) {
    const octets = ipv4.slice(1).map((value) => Number(value));
    if (octets[0] === 10) {
      return true;
    }
    if (octets[0] === 192 && octets[1] === 168) {
      return true;
    }
    if (octets[0] === 172 && octets[1] >= 16 && octets[1] <= 31) {
      return true;
    }
    if (octets[0] === 169 && octets[1] === 254) {
      return true;
    }
    if (octets[0] === 0) {
      return true;
    }
    if (octets[0] === 255 && octets[1] === 255 && octets[2] === 255 && octets[3] === 255) {
      return true;
    }
  }

  return false;
}

function isRetryableDelivery(delivery: { status: string; retryable?: boolean; status_code?: number }): boolean {
  if (delivery.status !== 'Failed' && delivery.status !== 'Retrying') {
    return false;
  }
  if (typeof delivery.retryable === 'boolean') {
    return delivery.retryable;
  }
  const statusCode = delivery.status_code;
  if (typeof statusCode !== 'number') {
    return true;
  }
  if (statusCode === 408 || statusCode === 425 || statusCode === 429) {
    return true;
  }
  return statusCode >= 500;
}

function validateChannelTarget(channelType: WebhookChannelType, target: string): string | undefined {
  const trimmed = target.trim();
  if (!trimmed) {
    return 'webhook.validation.urlRequired';
  }

  if (channelType === 'Telegram') {
    return isTelegramChatId(trimmed) ? undefined : 'webhook.validation.telegramTargetInvalid';
  }

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return 'webhook.validation.urlInvalid';
  }

  if (!parsed.hostname) {
    return 'webhook.validation.urlInvalid';
  }

  if (
    channelType === 'Slack' ||
    channelType === 'Feishu' ||
    channelType === 'Discord' ||
    channelType === 'ServerChan'
  ) {
    if (parsed.protocol !== 'https:') {
      return 'webhook.validation.httpsRequired';
    }
    return undefined;
  }

  if (channelType === 'Custom') {
    if (parsed.protocol === 'https:') {
      return undefined;
    }
    if (parsed.protocol === 'http:') {
      return isPrivateHost(parsed.hostname) ? undefined : 'webhook.validation.customHttpPrivateOnly';
    }
    return 'webhook.validation.customProtocolInvalid';
  }

  return undefined;
}

function formatDate(value?: string): string {
  if (!value) {
    return '-';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function parseScopeSessions(raw: string): string[] {
  return raw
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

export function WebhookSection() {
  const { t } = useTranslation('settings');
  const {
    channels,
    deliveries,
    health,
    loadingChannels,
    loadingDeliveries,
    loadingHealth,
    saving,
    testingByChannel,
    testResultsByChannel,
    error,
    deliveriesHasMore,
    fetchChannels,
    createChannel,
    updateChannel,
    setChannelEnabled,
    deleteChannel,
    testChannel,
    fetchDeliveries,
    retryDelivery,
    fetchHealth,
    clearChannelTestResult,
    clearError,
  } = useWebhookStore();

  const [showForm, setShowForm] = useState(false);
  const [editingChannel, setEditingChannel] = useState<WebhookChannelConfig | null>(null);

  const [formName, setFormName] = useState('');
  const [formType, setFormType] = useState<WebhookChannelType>('Slack');
  const [formUrl, setFormUrl] = useState('');
  const [formSecret, setFormSecret] = useState('');
  const [formScopeType, setFormScopeType] = useState<'global' | 'sessions'>('global');
  const [formScopeSessions, setFormScopeSessions] = useState('');
  const [formEvents, setFormEvents] = useState<WebhookEventType[]>(['TaskComplete', 'TaskFailed']);
  const [formTemplate, setFormTemplate] = useState('');
  const [formErrors, setFormErrors] = useState<FormErrors>({});

  const [historyFilterChannelId, setHistoryFilterChannelId] = useState<string>('all');
  const [historyPage, setHistoryPage] = useState(0);

  const historyChannelId = historyFilterChannelId === 'all' ? undefined : historyFilterChannelId;

  useEffect(() => {
    void fetchChannels();
    void fetchHealth();
  }, [fetchChannels, fetchHealth]);

  useEffect(() => {
    void fetchDeliveries(historyChannelId, PAGE_SIZE, historyPage * PAGE_SIZE);
  }, [fetchDeliveries, historyChannelId, historyPage]);

  const channelTypeForValidation = editingChannel?.channel_type ?? formType;

  const liveUrlError = useMemo(
    () => validateChannelTarget(channelTypeForValidation, formUrl),
    [channelTypeForValidation, formUrl],
  );

  const liveSessions = useMemo(() => parseScopeSessions(formScopeSessions), [formScopeSessions]);

  function resetForm() {
    setFormName('');
    setFormType('Slack');
    setFormUrl('');
    setFormSecret('');
    setFormScopeType('global');
    setFormScopeSessions('');
    setFormEvents(['TaskComplete', 'TaskFailed']);
    setFormTemplate('');
    setFormErrors({});
    setEditingChannel(null);
  }

  function openAddForm() {
    resetForm();
    setShowForm(true);
  }

  function openEditForm(channel: WebhookChannelConfig) {
    setFormName(channel.name);
    setFormType(channel.channel_type);
    setFormUrl(channel.url);
    setFormSecret('');
    if (channel.scope === 'Global') {
      setFormScopeType('global');
      setFormScopeSessions('');
    } else if (typeof channel.scope === 'object' && 'Sessions' in channel.scope) {
      setFormScopeType('sessions');
      setFormScopeSessions(channel.scope.Sessions.join(', '));
    }
    setFormEvents([...channel.events]);
    setFormTemplate(channel.template ?? '');
    setFormErrors({});
    setEditingChannel(channel);
    setShowForm(true);
  }

  function toggleEvent(event: WebhookEventType) {
    setFormEvents((prev) => (prev.includes(event) ? prev.filter((item) => item !== event) : [...prev, event]));
  }

  function buildScope(): WebhookScope {
    if (formScopeType === 'sessions') {
      return { Sessions: parseScopeSessions(formScopeSessions) };
    }
    return 'Global';
  }

  function validateForm(): FormErrors {
    const nextErrors: FormErrors = {};
    const trimmedName = formName.trim();
    const trimmedTemplate = formTemplate.trim();

    if (!trimmedName) {
      nextErrors.name = t('webhook.validation.nameRequired', 'Channel name is required');
    } else if (trimmedName.length > MAX_NAME_LEN) {
      nextErrors.name = t('webhook.validation.nameTooLong', {
        defaultValue: 'Channel name must be {{max}} characters or less',
        max: MAX_NAME_LEN,
      });
    }

    const urlErrorKey = validateChannelTarget(channelTypeForValidation, formUrl);
    if (urlErrorKey) {
      nextErrors.url = t(urlErrorKey);
    }
    if (!editingChannel && !formSecret.trim()) {
      if (channelTypeForValidation === 'Telegram') {
        nextErrors.secret = t('webhook.validation.telegramTokenRequired', 'Telegram channel requires a bot token');
      } else if (channelTypeForValidation === 'ServerChan') {
        nextErrors.secret = t('webhook.validation.serverchanSendkeyRequired', 'ServerChan channel requires SENDKEY');
      }
    }

    if (formEvents.length === 0) {
      nextErrors.events = t('webhook.validation.eventsRequired', 'Select at least one event');
    }

    if (formScopeType === 'sessions') {
      if (liveSessions.length === 0) {
        nextErrors.scopeSessions = t('webhook.validation.scopeRequired', 'Enter at least one session id');
      } else if (liveSessions.length > MAX_SCOPE_SESSIONS) {
        nextErrors.scopeSessions = t('webhook.validation.scopeTooManySessions', {
          defaultValue: 'At most {{max}} session ids are allowed',
          max: MAX_SCOPE_SESSIONS,
        });
      } else if (liveSessions.some((sessionId) => sessionId.length > MAX_SESSION_ID_LEN)) {
        nextErrors.scopeSessions = t('webhook.validation.scopeSessionTooLong', {
          defaultValue: 'Each session id must be {{max}} characters or less',
          max: MAX_SESSION_ID_LEN,
        });
      }
    }

    if (trimmedTemplate.length > MAX_TEMPLATE_LEN) {
      nextErrors.template = t('webhook.validation.templateTooLong', {
        defaultValue: 'Template must be {{max}} characters or less',
        max: MAX_TEMPLATE_LEN,
      });
    }

    return nextErrors;
  }

  async function handleSubmit() {
    const nextErrors = validateForm();
    setFormErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return;
    }

    if (editingChannel) {
      const request: UpdateWebhookRequest = {
        name: formName.trim(),
        url: formUrl.trim(),
        scope: buildScope(),
        events: [...formEvents],
      };
      const nextTemplate = formTemplate.trim();
      const previousTemplate = editingChannel.template?.trim() ?? '';
      if (nextTemplate !== previousTemplate) {
        request.template = nextTemplate ? nextTemplate : null;
      }

      if (formSecret.trim()) {
        request.secret = formSecret.trim();
      }

      const success = await updateChannel(editingChannel.id, request);
      if (!success) {
        return;
      }

      setShowForm(false);
      resetForm();
      return;
    }

    const request: CreateWebhookRequest = {
      name: formName.trim(),
      channel_type: formType,
      url: formUrl.trim(),
      secret: formSecret.trim() || undefined,
      scope: buildScope(),
      events: [...formEvents],
      template: formTemplate.trim() || undefined,
    };

    const success = await createChannel(request);
    if (!success) {
      return;
    }

    setShowForm(false);
    resetForm();
  }

  async function handleDelete(channelId: string) {
    await deleteChannel(channelId);
  }

  async function handleRetry(deliveryId: string) {
    const ok = await retryDelivery(deliveryId);
    if (ok) {
      void fetchHealth();
    }
  }

  async function handleToggleEnabled(channelId: string, enabled: boolean) {
    const ok = await setChannelEnabled(channelId, enabled);
    if (ok) {
      void fetchHealth();
    }
  }

  function statusText(status: string): string {
    if (status === 'Success') {
      return t('webhook.deliveryStatus.success', 'Success');
    }
    if (status === 'Failed') {
      return t('webhook.deliveryStatus.failed', 'Failed');
    }
    if (status === 'Retrying') {
      return t('webhook.deliveryStatus.retrying', 'Retrying');
    }
    return t('webhook.deliveryStatus.pending', 'Pending');
  }

  function statusClass(status: string): string {
    if (status === 'Success') {
      return 'text-green-700 bg-green-100 dark:text-green-300 dark:bg-green-900/30';
    }
    if (status === 'Failed') {
      return 'text-red-700 bg-red-100 dark:text-red-300 dark:bg-red-900/30';
    }
    if (status === 'Retrying') {
      return 'text-amber-700 bg-amber-100 dark:text-amber-300 dark:bg-amber-900/30';
    }
    return 'text-slate-700 bg-slate-100 dark:text-slate-300 dark:bg-slate-800';
  }

  function channelIcon(type: WebhookChannelType) {
    switch (type) {
      case 'Slack':
        return 'S';
      case 'Feishu':
        return 'F';
      case 'Telegram':
        return 'T';
      case 'ServerChan':
        return 'SC';
      case 'Discord':
        return 'D';
      case 'Custom':
        return 'C';
    }
  }

  function scopeDisplay(scope: WebhookScope) {
    if (scope === 'Global') {
      return t('webhook.scope.global', 'Global');
    }
    if (typeof scope === 'object' && 'Sessions' in scope) {
      return t('webhook.scope.sessionsCount', {
        defaultValue: '{{count}} sessions',
        count: scope.Sessions.length,
      });
    }
    return t('webhook.scope.global', 'Global');
  }

  function timeAgo(value: string): string {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return value;
    }

    const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
    if (seconds < 60) {
      return t('webhook.time.justNow', 'just now');
    }
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) {
      return t('webhook.time.minutesAgo', {
        defaultValue: '{{count}}m ago',
        count: minutes,
      });
    }
    const hours = Math.floor(minutes / 60);
    if (hours < 24) {
      return t('webhook.time.hoursAgo', {
        defaultValue: '{{count}}h ago',
        count: hours,
      });
    }
    const days = Math.floor(hours / 24);
    return t('webhook.time.daysAgo', {
      defaultValue: '{{count}}d ago',
      count: days,
    });
  }

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {t('webhook.title', 'Notifications')}
        </h3>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          {t('webhook.description', 'Configure webhook channels to receive notifications when tasks complete or fail.')}
        </p>
      </div>

      {error && (
        <div className="rounded-md bg-red-50 dark:bg-red-900/20 p-3">
          <p className="text-sm text-red-700 dark:text-red-400">{error}</p>
          <button onClick={clearError} className="text-xs text-red-600 dark:text-red-300 underline mt-1">
            {t('webhook.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4">
        <div className="flex items-center justify-between gap-3">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">
            {t('webhook.health.title', 'Delivery Worker Health')}
          </h4>
          <button
            onClick={() => {
              void fetchHealth();
            }}
            className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            {loadingHealth ? t('webhook.loading', 'Loading...') : t('webhook.health.refresh', 'Refresh')}
          </button>
        </div>

        <div className="mt-3 grid grid-cols-1 md:grid-cols-3 gap-3 text-sm">
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <p className="text-gray-500 dark:text-gray-400">{t('webhook.health.workerStatus', 'Worker')}</p>
            <p className="mt-1 font-medium text-gray-900 dark:text-gray-100">
              {health?.worker_running
                ? t('webhook.health.workerRunning', 'Running')
                : t('webhook.health.workerStopped', 'Stopped')}
            </p>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <p className="text-gray-500 dark:text-gray-400">{t('webhook.health.failedQueue', 'Failed Queue')}</p>
            <p className="mt-1 font-medium text-gray-900 dark:text-gray-100">{health?.failed_queue_length ?? '-'}</p>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <p className="text-gray-500 dark:text-gray-400">{t('webhook.health.lastRetryAt', 'Last Retry')}</p>
            <p className="mt-1 font-medium text-gray-900 dark:text-gray-100">{formatDate(health?.last_retry_at)}</p>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <p className="text-gray-500 dark:text-gray-400">
              {t('webhook.health.persistenceFailures', 'Persistence Failures')}
            </p>
            <p className="mt-1 font-medium text-gray-900 dark:text-gray-100">{health?.persistence_failures ?? '-'}</p>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <p className="text-gray-500 dark:text-gray-400">{t('webhook.health.retryCycles', 'Retry Cycles')}</p>
            <p className="mt-1 font-medium text-gray-900 dark:text-gray-100">{health?.retry_cycle_count ?? '-'}</p>
          </div>
          <div className="rounded border border-gray-200 dark:border-gray-700 p-3">
            <p className="text-gray-500 dark:text-gray-400">{t('webhook.health.lastRetryError', 'Last Retry Error')}</p>
            <p className="mt-1 font-medium text-gray-900 dark:text-gray-100 truncate">
              {health?.last_retry_error ?? '-'}
            </p>
          </div>
        </div>
      </div>

      <div className="border border-gray-200 dark:border-gray-700 rounded-lg">
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">{t('webhook.channels', 'Webhook Channels')}</h4>
          <button
            onClick={openAddForm}
            className="text-sm px-3 py-1 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
          >
            + {t('webhook.add', 'Add')}
          </button>
        </div>

        {loadingChannels && channels.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.loading', 'Loading...')}
          </div>
        ) : channels.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.noChannels', 'No webhook channels configured.')}
          </div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {channels.map((channel) => {
              const testResult = testResultsByChannel[channel.id];
              const testing = !!testingByChannel[channel.id];
              return (
                <div key={channel.id} className="px-4 py-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-3 min-w-0">
                      <span
                        className={clsx(
                          'w-2 h-2 rounded-full flex-shrink-0',
                          channel.enabled ? 'bg-green-500' : 'bg-gray-400',
                        )}
                      />
                      <span className="w-6 h-6 flex items-center justify-center text-xs font-bold bg-gray-100 dark:bg-gray-800 rounded flex-shrink-0">
                        {channelIcon(channel.channel_type)}
                      </span>
                      <div className="min-w-0">
                        <p className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">{channel.name}</p>
                        <p className="text-xs text-gray-500 dark:text-gray-400 truncate">
                          {channel.events.length} {t('webhook.eventsLabel', 'events')} | {scopeDisplay(channel.scope)}
                        </p>
                      </div>
                    </div>

                    <div className="flex items-center gap-2 flex-shrink-0">
                      <label className="inline-flex items-center gap-2 text-xs text-gray-600 dark:text-gray-300">
                        {t('webhook.enabled', 'Enabled')}
                        <input
                          type="checkbox"
                          checked={channel.enabled}
                          onChange={(event) => {
                            void handleToggleEnabled(channel.id, event.target.checked);
                          }}
                          disabled={saving}
                          aria-label={`${channel.name}-enabled`}
                        />
                      </label>

                      <button
                        onClick={() => {
                          void testChannel(channel.id);
                        }}
                        disabled={testing}
                        className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors disabled:opacity-50"
                      >
                        {testing ? t('webhook.testing', 'Testing...') : t('webhook.test', 'Test')}
                      </button>
                      <button
                        onClick={() => openEditForm(channel)}
                        className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
                      >
                        {t('webhook.edit', 'Edit')}
                      </button>
                      <button
                        onClick={() => {
                          void handleDelete(channel.id);
                        }}
                        className="text-xs px-2 py-1 border border-red-300 dark:border-red-600 text-red-600 dark:text-red-400 rounded hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                      >
                        {t('webhook.delete', 'Delete')}
                      </button>
                    </div>
                  </div>

                  {testResult && (
                    <div
                      className={clsx(
                        'mt-2 rounded p-2 text-xs flex items-center justify-between gap-2',
                        testResult.success
                          ? 'bg-green-50 text-green-700 dark:bg-green-900/20 dark:text-green-300'
                          : 'bg-red-50 text-red-700 dark:bg-red-900/20 dark:text-red-300',
                      )}
                    >
                      <span>
                        {testResult.success
                          ? `${t('webhook.testSuccess', 'Test successful')}${testResult.latency_ms ? ` (${testResult.latency_ms}ms)` : ''}`
                          : `${t('webhook.testFailed', 'Test failed')}: ${testResult.error ?? t('webhook.unknownError', 'Unknown error')}`}
                      </span>
                      <button
                        onClick={() => clearChannelTestResult(channel.id)}
                        className="underline"
                        aria-label={`${channel.name}-dismiss-test`}
                      >
                        {t('webhook.dismiss', 'Dismiss')}
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {showForm && (
        <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 space-y-4">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">
            {editingChannel ? t('webhook.editChannel', 'Edit Channel') : t('webhook.addChannel', 'Add Channel')}
          </h4>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.name', 'Name')}
            </label>
            <input
              type="text"
              value={formName}
              onChange={(event) => {
                setFormName(event.target.value);
                if (formErrors.name) {
                  setFormErrors((prev) => ({ ...prev, name: undefined }));
                }
              }}
              placeholder={t('webhook.form.namePlaceholder', 'e.g., Team Slack')}
              maxLength={MAX_NAME_LEN + 1}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
            />
            {formErrors.name && <p className="mt-1 text-xs text-red-600 dark:text-red-400">{formErrors.name}</p>}
          </div>

          {!editingChannel && (
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('webhook.form.channelType', 'Channel Type')}
              </label>
              <select
                value={formType}
                onChange={(event) => {
                  setFormType(event.target.value as WebhookChannelType);
                  setFormErrors((prev) => ({ ...prev, url: undefined }));
                }}
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
              >
                {CHANNEL_TYPES.map((channelType) => (
                  <option key={channelType.value} value={channelType.value}>
                    {channelType.label}
                  </option>
                ))}
              </select>
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.url', 'Webhook URL')}
            </label>
            <input
              type="text"
              value={formUrl}
              onChange={(event) => {
                setFormUrl(event.target.value);
                setFormErrors((prev) => ({ ...prev, url: undefined }));
              }}
              placeholder={
                channelTypeForValidation === 'Telegram'
                  ? t('webhook.form.telegramPlaceholder', 'e.g., -1001234567890 or @channel_name')
                  : channelTypeForValidation === 'ServerChan'
                    ? t('webhook.form.serverchanPlaceholder', 'e.g., https://sctapi.ftqq.com')
                    : 'https://hooks.example.com/...'
              }
              className={clsx(
                'w-full px-3 py-2 border rounded bg-white dark:bg-gray-800 text-sm',
                formErrors.url ? 'border-red-400 dark:border-red-600' : 'border-gray-300 dark:border-gray-600',
              )}
            />
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {channelTypeForValidation === 'Telegram'
                ? t('webhook.form.telegramHelp', 'Telegram channel expects chat_id or @name')
                : channelTypeForValidation === 'ServerChan'
                  ? t('webhook.form.serverchanHelp', 'ServerChan channel expects API base URL; put SENDKEY in Secret')
                  : t('webhook.form.urlHelp', 'Use HTTPS endpoints for production channels')}
            </p>
            {(formErrors.url || (liveUrlError ? t(liveUrlError) : '')) && (
              <p className="mt-1 text-xs text-red-600 dark:text-red-400">
                {formErrors.url || t(liveUrlError as string)}
              </p>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.secret', 'Secret / Token')}
            </label>
            <input
              type="password"
              value={formSecret}
              onChange={(event) => {
                setFormSecret(event.target.value);
                if (formErrors.secret) {
                  setFormErrors((prev) => ({ ...prev, secret: undefined }));
                }
              }}
              placeholder={
                editingChannel
                  ? t('webhook.form.secretPlaceholder', 'Leave empty to keep current')
                  : t('webhook.form.secretOptional', 'Optional')
              }
              className={clsx(
                'w-full px-3 py-2 border rounded bg-white dark:bg-gray-800 text-sm',
                formErrors.secret ? 'border-red-400 dark:border-red-600' : 'border-gray-300 dark:border-gray-600',
              )}
            />
            {formErrors.secret && <p className="mt-1 text-xs text-red-600 dark:text-red-400">{formErrors.secret}</p>}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.scope', 'Scope')}
            </label>
            <div className="flex items-center gap-4">
              <label className="flex items-center gap-1 text-sm">
                <input
                  type="radio"
                  checked={formScopeType === 'global'}
                  onChange={() => {
                    setFormScopeType('global');
                    setFormErrors((prev) => ({ ...prev, scopeSessions: undefined }));
                  }}
                />
                {t('webhook.scope.global', 'Global')}
              </label>
              <label className="flex items-center gap-1 text-sm">
                <input
                  type="radio"
                  checked={formScopeType === 'sessions'}
                  onChange={() => setFormScopeType('sessions')}
                />
                {t('webhook.scope.specificSessions', 'Specific Sessions')}
              </label>
            </div>
            {formScopeType === 'sessions' && (
              <>
                <input
                  type="text"
                  value={formScopeSessions}
                  onChange={(event) => {
                    setFormScopeSessions(event.target.value);
                    setFormErrors((prev) => ({ ...prev, scopeSessions: undefined }));
                  }}
                  placeholder={t('webhook.form.sessionIds', 'Comma-separated session IDs')}
                  className={clsx(
                    'mt-2 w-full px-3 py-2 border rounded bg-white dark:bg-gray-800 text-sm',
                    formErrors.scopeSessions
                      ? 'border-red-400 dark:border-red-600'
                      : 'border-gray-300 dark:border-gray-600',
                  )}
                />
                <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                  {t('webhook.form.scopeLimitHint', {
                    defaultValue: 'Up to {{max}} session IDs',
                    max: MAX_SCOPE_SESSIONS,
                  })}
                </p>
                {formErrors.scopeSessions && (
                  <p className="mt-1 text-xs text-red-600 dark:text-red-400">{formErrors.scopeSessions}</p>
                )}
              </>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.events', 'Events')}
            </label>
            <div className="grid grid-cols-2 gap-2">
              {EVENT_TYPES.map((eventType) => (
                <label key={eventType.value} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={formEvents.includes(eventType.value)}
                    onChange={() => {
                      toggleEvent(eventType.value);
                      setFormErrors((prev) => ({ ...prev, events: undefined }));
                    }}
                  />
                  {t(eventType.labelKey, eventType.value)}
                </label>
              ))}
            </div>
            {formErrors.events && <p className="mt-1 text-xs text-red-600 dark:text-red-400">{formErrors.events}</p>}
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.template', 'Custom Template')} ({t('webhook.form.optional', 'optional')})
            </label>
            <textarea
              value={formTemplate}
              onChange={(event) => {
                setFormTemplate(event.target.value);
                if (formErrors.template) {
                  setFormErrors((prev) => ({ ...prev, template: undefined }));
                }
              }}
              rows={4}
              className={clsx(
                'w-full px-3 py-2 border rounded bg-white dark:bg-gray-800 text-sm',
                formErrors.template ? 'border-red-400 dark:border-red-600' : 'border-gray-300 dark:border-gray-600',
              )}
            />
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
              {t(
                'webhook.form.templateHelp',
                'Template only overrides summary text; unsupported placeholders fallback safely.',
              )}
            </p>
            {formErrors.template && (
              <p className="mt-1 text-xs text-red-600 dark:text-red-400">{formErrors.template}</p>
            )}
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={() => {
                void handleSubmit();
              }}
              disabled={saving}
              className="px-4 py-2 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors disabled:opacity-50"
            >
              {saving
                ? t('webhook.saving', 'Saving...')
                : editingChannel
                  ? t('webhook.update', 'Update')
                  : t('webhook.create', 'Create')}
            </button>
            <button
              onClick={() => {
                setShowForm(false);
                resetForm();
              }}
              className="px-4 py-2 border border-gray-300 dark:border-gray-600 text-sm rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
            >
              {t('webhook.cancel', 'Cancel')}
            </button>
          </div>
        </div>
      )}

      <div className="border border-gray-200 dark:border-gray-700 rounded-lg">
        <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 space-y-3">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">
            {t('webhook.deliveryHistory', 'Delivery History')}
          </h4>
          <div className="flex items-center gap-3">
            <label className="text-sm text-gray-600 dark:text-gray-300">{t('webhook.filter.channel', 'Channel')}</label>
            <select
              value={historyFilterChannelId}
              onChange={(event) => {
                setHistoryFilterChannelId(event.target.value);
                setHistoryPage(0);
              }}
              className="px-2 py-1 text-sm border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800"
            >
              <option value="all">{t('webhook.filter.allChannels', 'All channels')}</option>
              {channels.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  {channel.name}
                </option>
              ))}
            </select>
            <button
              onClick={() => {
                void fetchDeliveries(historyChannelId, PAGE_SIZE, historyPage * PAGE_SIZE);
              }}
              className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800"
            >
              {t('webhook.refresh', 'Refresh')}
            </button>
          </div>
        </div>

        {loadingDeliveries && deliveries.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.loading', 'Loading...')}
          </div>
        ) : deliveries.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.noDeliveries', 'No delivery history yet.')}
          </div>
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="min-w-full text-sm">
                <thead className="bg-gray-50 dark:bg-gray-800/50 text-gray-600 dark:text-gray-300">
                  <tr>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.event', 'Event')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.status', 'Status')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.attempts', 'Attempts')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.statusCode', 'Status Code')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.lastError', 'Last Error')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.nextRetryAt', 'Next Retry')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.createdAt', 'Created')}</th>
                    <th className="px-4 py-2 text-left font-medium">{t('webhook.table.actions', 'Actions')}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                  {deliveries.map((delivery) => (
                    <tr key={delivery.id}>
                      <td className="px-4 py-2 align-top">
                        <p className="text-gray-900 dark:text-gray-100">{delivery.event_type}</p>
                        <p className="text-xs text-gray-500 dark:text-gray-400 max-w-sm truncate">
                          {delivery.payload.summary}
                        </p>
                      </td>
                      <td className="px-4 py-2 align-top">
                        <span className={clsx('inline-flex px-2 py-0.5 rounded text-xs', statusClass(delivery.status))}>
                          {statusText(delivery.status)}
                        </span>
                      </td>
                      <td className="px-4 py-2 align-top text-gray-700 dark:text-gray-300">{delivery.attempts}</td>
                      <td className="px-4 py-2 align-top text-gray-700 dark:text-gray-300">
                        {delivery.status_code ?? '-'}
                      </td>
                      <td className="px-4 py-2 align-top text-xs text-red-700 dark:text-red-300 max-w-sm truncate">
                        {delivery.last_error ?? '-'}
                      </td>
                      <td className="px-4 py-2 align-top text-gray-700 dark:text-gray-300">
                        {formatDate(delivery.next_retry_at)}
                      </td>
                      <td
                        className="px-4 py-2 align-top text-gray-700 dark:text-gray-300"
                        title={formatDate(delivery.created_at)}
                      >
                        {timeAgo(delivery.created_at)}
                      </td>
                      <td className="px-4 py-2 align-top">
                        {isRetryableDelivery(delivery) && (
                          <button
                            onClick={() => {
                              void handleRetry(delivery.id);
                            }}
                            disabled={saving}
                            className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors disabled:opacity-50"
                          >
                            {t('webhook.retry', 'Retry')}
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            <div className="flex items-center justify-between px-4 py-3 border-t border-gray-200 dark:border-gray-700 text-sm">
              <p className="text-gray-500 dark:text-gray-400">
                {t('webhook.pagination.page', {
                  defaultValue: 'Page {{page}}',
                  page: historyPage + 1,
                })}
              </p>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setHistoryPage((value) => Math.max(0, value - 1))}
                  disabled={historyPage === 0 || loadingDeliveries}
                  className="px-2 py-1 border border-gray-300 dark:border-gray-600 rounded disabled:opacity-50"
                >
                  {t('webhook.pagination.previous', 'Previous')}
                </button>
                <button
                  onClick={() => setHistoryPage((value) => value + 1)}
                  disabled={!deliveriesHasMore || loadingDeliveries}
                  className="px-2 py-1 border border-gray-300 dark:border-gray-600 rounded disabled:opacity-50"
                >
                  {t('webhook.pagination.next', 'Next')}
                </button>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
