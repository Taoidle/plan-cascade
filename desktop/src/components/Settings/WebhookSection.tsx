/**
 * WebhookSection Component
 *
 * Settings section for managing webhook notification channels.
 * Includes channel list, add/edit forms, test buttons, and delivery history.
 */

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useWebhookStore } from '../../store/webhook';
import type {
  WebhookChannelType,
  WebhookScope,
  WebhookEventType,
  CreateWebhookRequest,
  UpdateWebhookRequest,
  WebhookChannelConfig,
} from '../../lib/webhookApi';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CHANNEL_TYPES: { value: WebhookChannelType; label: string }[] = [
  { value: 'Slack', label: 'Slack' },
  { value: 'Feishu', label: 'Feishu / Lark' },
  { value: 'Telegram', label: 'Telegram' },
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

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function WebhookSection() {
  const { t } = useTranslation('settings');
  const {
    channels,
    deliveries,
    loading,
    saving,
    testing,
    testResult,
    error,
    fetchChannels,
    createChannel,
    updateChannel,
    deleteChannel,
    testChannel,
    fetchDeliveries,
    retryDelivery,
    clearTestResult,
    clearError,
  } = useWebhookStore();

  const [showForm, setShowForm] = useState(false);
  const [editingChannel, setEditingChannel] = useState<WebhookChannelConfig | null>(null);

  // Form state
  const [formName, setFormName] = useState('');
  const [formType, setFormType] = useState<WebhookChannelType>('Slack');
  const [formUrl, setFormUrl] = useState('');
  const [formSecret, setFormSecret] = useState('');
  const [formScopeType, setFormScopeType] = useState<'global' | 'sessions'>('global');
  const [formScopeSessions, setFormScopeSessions] = useState('');
  const [formEvents, setFormEvents] = useState<WebhookEventType[]>(['TaskComplete', 'TaskFailed']);
  const [formTemplate, setFormTemplate] = useState('');

  useEffect(() => {
    fetchChannels();
    fetchDeliveries();
  }, [fetchChannels, fetchDeliveries]);

  // Reset form
  const resetForm = () => {
    setFormName('');
    setFormType('Slack');
    setFormUrl('');
    setFormSecret('');
    setFormScopeType('global');
    setFormScopeSessions('');
    setFormEvents(['TaskComplete', 'TaskFailed']);
    setFormTemplate('');
    setEditingChannel(null);
    clearTestResult();
  };

  // Open add form
  const handleAdd = () => {
    resetForm();
    setShowForm(true);
  };

  // Open edit form
  const handleEdit = (channel: WebhookChannelConfig) => {
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
    setEditingChannel(channel);
    setShowForm(true);
  };

  // Toggle event in form
  const toggleEvent = (event: WebhookEventType) => {
    setFormEvents((prev) =>
      prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event]
    );
  };

  // Build scope from form state
  const buildScope = (): WebhookScope => {
    if (formScopeType === 'sessions' && formScopeSessions.trim()) {
      return {
        Sessions: formScopeSessions
          .split(',')
          .map((s) => s.trim())
          .filter(Boolean),
      };
    }
    return 'Global';
  };

  // Submit form
  const handleSubmit = async () => {
    if (!formName.trim() || !formUrl.trim() || formEvents.length === 0) {
      return;
    }

    if (editingChannel) {
      const request: UpdateWebhookRequest = {
        name: formName,
        url: formUrl,
        scope: buildScope(),
        events: formEvents,
        template: formTemplate || undefined,
      };
      if (formSecret) {
        request.secret = formSecret;
      }
      const success = await updateChannel(editingChannel.id, request);
      if (success) {
        setShowForm(false);
        resetForm();
      }
    } else {
      const request: CreateWebhookRequest = {
        name: formName,
        channel_type: formType,
        url: formUrl,
        secret: formSecret || undefined,
        scope: buildScope(),
        events: formEvents,
        template: formTemplate || undefined,
      };
      const success = await createChannel(request);
      if (success) {
        setShowForm(false);
        resetForm();
      }
    }
  };

  // Handle delete
  const handleDelete = async (id: string) => {
    await deleteChannel(id);
  };

  // Handle test
  const handleTest = async (id: string) => {
    await testChannel(id);
  };

  // Handle retry delivery
  const handleRetry = async (deliveryId: string) => {
    await retryDelivery(deliveryId);
  };

  // Format time ago
  const timeAgo = (dateStr: string) => {
    const diff = Date.now() - new Date(dateStr).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
  };

  // Get channel type icon
  const channelIcon = (type: WebhookChannelType) => {
    switch (type) {
      case 'Slack': return '#';
      case 'Feishu': return 'F';
      case 'Telegram': return 'T';
      case 'Discord': return 'D';
      case 'Custom': return 'H';
    }
  };

  // Get scope display
  const scopeDisplay = (scope: WebhookScope) => {
    if (scope === 'Global') return t('webhook.scope.global', 'Global');
    if (typeof scope === 'object' && 'Sessions' in scope) {
      return `${scope.Sessions.length} ${t('webhook.scope.sessions', 'sessions')}`;
    }
    return 'Global';
  };

  return (
    <div className="space-y-6">
      {/* Section Header */}
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {t('webhook.title', 'Notifications')}
        </h3>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          {t('webhook.description', 'Configure webhook channels to receive notifications when tasks complete or fail.')}
        </p>
      </div>

      {/* Error display */}
      {error && (
        <div className="rounded-md bg-red-50 dark:bg-red-900/20 p-3">
          <p className="text-sm text-red-700 dark:text-red-400">{error}</p>
          <button
            onClick={clearError}
            className="text-xs text-red-600 dark:text-red-300 underline mt-1"
          >
            {t('webhook.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {/* Test result display */}
      {testResult && (
        <div
          className={clsx(
            'rounded-md p-3',
            testResult.success
              ? 'bg-green-50 dark:bg-green-900/20'
              : 'bg-red-50 dark:bg-red-900/20'
          )}
        >
          <p
            className={clsx(
              'text-sm',
              testResult.success
                ? 'text-green-700 dark:text-green-400'
                : 'text-red-700 dark:text-red-400'
            )}
          >
            {testResult.success
              ? `${t('webhook.testSuccess', 'Test successful')}${testResult.latency_ms ? ` (${testResult.latency_ms}ms)` : ''}`
              : `${t('webhook.testFailed', 'Test failed')}: ${testResult.error ?? 'Unknown error'}`}
          </p>
          <button
            onClick={clearTestResult}
            className="text-xs underline mt-1 text-gray-600 dark:text-gray-300"
          >
            {t('webhook.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {/* Channel List */}
      <div className="border border-gray-200 dark:border-gray-700 rounded-lg">
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">
            {t('webhook.channels', 'Webhook Channels')}
          </h4>
          <button
            onClick={handleAdd}
            className="text-sm px-3 py-1 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
          >
            + {t('webhook.add', 'Add')}
          </button>
        </div>

        {loading && channels.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.loading', 'Loading...')}
          </div>
        ) : channels.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.noChannels', 'No webhook channels configured.')}
          </div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {channels.map((channel) => (
              <div
                key={channel.id}
                className="px-4 py-3 flex items-center justify-between"
              >
                <div className="flex items-center gap-3 min-w-0">
                  <span
                    className={clsx(
                      'w-2 h-2 rounded-full flex-shrink-0',
                      channel.enabled ? 'bg-green-500' : 'bg-gray-400'
                    )}
                  />
                  <span className="w-6 h-6 flex items-center justify-center text-xs font-bold bg-gray-100 dark:bg-gray-800 rounded flex-shrink-0">
                    {channelIcon(channel.channel_type)}
                  </span>
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
                      {channel.name}
                    </p>
                    <p className="text-xs text-gray-500 dark:text-gray-400 truncate">
                      {channel.events.length} {t('webhook.eventsLabel', 'events')} | {scopeDisplay(channel.scope)}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <button
                    onClick={() => handleTest(channel.id)}
                    disabled={testing}
                    className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors disabled:opacity-50"
                  >
                    {testing ? '...' : t('webhook.test', 'Test')}
                  </button>
                  <button
                    onClick={() => handleEdit(channel)}
                    className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
                  >
                    {t('webhook.edit', 'Edit')}
                  </button>
                  <button
                    onClick={() => handleDelete(channel.id)}
                    className="text-xs px-2 py-1 border border-red-300 dark:border-red-600 text-red-600 dark:text-red-400 rounded hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                  >
                    {t('webhook.delete', 'Delete')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Add/Edit Form */}
      {showForm && (
        <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 space-y-4">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">
            {editingChannel
              ? t('webhook.editChannel', 'Edit Channel')
              : t('webhook.addChannel', 'Add Channel')}
          </h4>

          {/* Name */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.name', 'Name')}
            </label>
            <input
              type="text"
              value={formName}
              onChange={(e) => setFormName(e.target.value)}
              placeholder="e.g., Dev Team Slack"
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
            />
          </div>

          {/* Channel Type (only for new) */}
          {!editingChannel && (
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('webhook.form.channelType', 'Channel Type')}
              </label>
              <select
                value={formType}
                onChange={(e) => setFormType(e.target.value as WebhookChannelType)}
                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
              >
                {CHANNEL_TYPES.map((ct) => (
                  <option key={ct.value} value={ct.value}>
                    {ct.label}
                  </option>
                ))}
              </select>
            </div>
          )}

          {/* URL */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.url', 'Webhook URL')}
            </label>
            <input
              type="text"
              value={formUrl}
              onChange={(e) => setFormUrl(e.target.value)}
              placeholder="https://hooks.slack.com/services/..."
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
            />
          </div>

          {/* Secret */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.secret', 'Secret / Token')}
            </label>
            <input
              type="password"
              value={formSecret}
              onChange={(e) => setFormSecret(e.target.value)}
              placeholder={editingChannel ? t('webhook.form.secretPlaceholder', 'Leave empty to keep current') : t('webhook.form.secretOptional', 'Optional')}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
            />
          </div>

          {/* Scope */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.scope', 'Scope')}
            </label>
            <div className="flex items-center gap-4">
              <label className="flex items-center gap-1 text-sm">
                <input
                  type="radio"
                  checked={formScopeType === 'global'}
                  onChange={() => setFormScopeType('global')}
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
              <input
                type="text"
                value={formScopeSessions}
                onChange={(e) => setFormScopeSessions(e.target.value)}
                placeholder={t('webhook.form.sessionIds', 'Comma-separated session IDs')}
                className="mt-2 w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
              />
            )}
          </div>

          {/* Events */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.events', 'Events')}
            </label>
            <div className="grid grid-cols-2 gap-2">
              {EVENT_TYPES.map((evt) => (
                <label key={evt.value} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={formEvents.includes(evt.value)}
                    onChange={() => toggleEvent(evt.value)}
                  />
                  {t(evt.labelKey, evt.value)}
                </label>
              ))}
            </div>
          </div>

          {/* Template */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('webhook.form.template', 'Custom Template')} ({t('webhook.form.optional', 'optional')})
            </label>
            <textarea
              value={formTemplate}
              onChange={(e) => setFormTemplate(e.target.value)}
              rows={3}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-sm"
            />
          </div>

          {/* Form Actions */}
          <div className="flex items-center gap-3">
            <button
              onClick={handleSubmit}
              disabled={saving || !formName.trim() || !formUrl.trim() || formEvents.length === 0}
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

      {/* Delivery History */}
      <div className="border border-gray-200 dark:border-gray-700 rounded-lg">
        <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
          <h4 className="font-medium text-gray-900 dark:text-gray-100">
            {t('webhook.deliveryHistory', 'Delivery History')}
          </h4>
        </div>

        {deliveries.length === 0 ? (
          <div className="p-4 text-center text-gray-500 dark:text-gray-400 text-sm">
            {t('webhook.noDeliveries', 'No delivery history yet.')}
          </div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {deliveries.slice(0, 10).map((delivery) => (
              <div
                key={delivery.id}
                className="px-4 py-2 flex items-center justify-between text-sm"
              >
                <div className="flex items-center gap-3 min-w-0">
                  <span
                    className={clsx(
                      'w-4 text-center flex-shrink-0',
                      delivery.status === 'Success'
                        ? 'text-green-600'
                        : delivery.status === 'Failed'
                          ? 'text-red-600'
                          : 'text-yellow-600'
                    )}
                  >
                    {delivery.status === 'Success' ? 'OK' : delivery.status === 'Failed' ? 'ERR' : '...'}
                  </span>
                  <span className="text-gray-500 dark:text-gray-400 text-xs flex-shrink-0">
                    {timeAgo(delivery.created_at)}
                  </span>
                  <span className="text-gray-700 dark:text-gray-300 truncate">
                    {delivery.event_type}
                  </span>
                  <span className="text-gray-500 dark:text-gray-400 truncate text-xs">
                    {delivery.payload.summary.substring(0, 50)}
                  </span>
                </div>
                {delivery.status === 'Failed' && (
                  <button
                    onClick={() => handleRetry(delivery.id)}
                    disabled={saving}
                    className="text-xs px-2 py-1 border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors flex-shrink-0"
                  >
                    {t('webhook.retry', 'Retry')}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
