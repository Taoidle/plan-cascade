/**
 * Period Selector Component
 *
 * Provides preset period options and custom date range selection.
 */

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import * as Select from '@radix-ui/react-select';
import { clsx } from 'clsx';
import { useAnalyticsStore, type PeriodPreset, type AggregationPeriod } from '../../store/analytics';

interface PeriodSelectorProps {
  onChange?: () => void;
}

export function PeriodSelector({ onChange }: PeriodSelectorProps) {
  const { t } = useTranslation('analytics');
  const [showCustomRange, setShowCustomRange] = useState(false);

  const { periodPreset, period, setPeriodPreset, setPeriod, setDateRange } = useAnalyticsStore();

  const presetOptions: { value: PeriodPreset; label: string }[] = [
    { value: 'last7days', label: t('periods.last7days', 'Last 7 Days') },
    { value: 'last30days', label: t('periods.last30days', 'Last 30 Days') },
    { value: 'last90days', label: t('periods.last90days', 'Last 90 Days') },
    { value: 'custom', label: t('periods.custom', 'Custom Range') },
  ];

  const aggregationOptions: { value: AggregationPeriod; label: string }[] = [
    { value: 'hourly', label: t('aggregation.hourly', 'Hourly') },
    { value: 'daily', label: t('aggregation.daily', 'Daily') },
    { value: 'weekly', label: t('aggregation.weekly', 'Weekly') },
    { value: 'monthly', label: t('aggregation.monthly', 'Monthly') },
  ];

  const handlePresetChange = (value: PeriodPreset) => {
    if (value === 'custom') {
      setShowCustomRange(true);
    } else {
      setShowCustomRange(false);
      setPeriodPreset(value);
      onChange?.();
    }
  };

  const handleAggregationChange = (value: AggregationPeriod) => {
    setPeriod(value);
    onChange?.();
  };

  const handleCustomRangeSubmit = (start: Date, end: Date) => {
    setDateRange(start, end);
    setShowCustomRange(false);
    onChange?.();
  };

  return (
    <div className="flex items-center gap-3">
      {/* Period Preset Selector */}
      <Select.Root value={periodPreset} onValueChange={handlePresetChange}>
        <Select.Trigger
          className={clsx(
            'inline-flex items-center justify-between',
            'px-3 py-2 min-w-[160px] rounded-lg',
            'bg-white dark:bg-gray-800',
            'border border-gray-300 dark:border-gray-700',
            'text-sm text-gray-900 dark:text-white',
            'hover:bg-gray-50 dark:hover:bg-gray-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          <Select.Value />
          <Select.Icon>
            <svg className="w-4 h-4 ml-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </Select.Icon>
        </Select.Trigger>

        <Select.Portal>
          <Select.Content
            className={clsx(
              'overflow-hidden rounded-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'shadow-lg'
            )}
          >
            <Select.Viewport className="p-1">
              {presetOptions.map((option) => (
                <Select.Item
                  key={option.value}
                  value={option.value}
                  className={clsx(
                    'relative flex items-center px-8 py-2 text-sm',
                    'text-gray-900 dark:text-white',
                    'rounded cursor-pointer',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                    'focus:outline-none focus:bg-gray-100 dark:focus:bg-gray-700',
                    'data-[state=checked]:bg-primary-50 dark:data-[state=checked]:bg-primary-900/30'
                  )}
                >
                  <Select.ItemText>{option.label}</Select.ItemText>
                  <Select.ItemIndicator className="absolute left-2">
                    <svg className="w-4 h-4 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                      <path
                        fillRule="evenodd"
                        d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                        clipRule="evenodd"
                      />
                    </svg>
                  </Select.ItemIndicator>
                </Select.Item>
              ))}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>

      {/* Aggregation Period Selector */}
      <Select.Root value={period} onValueChange={handleAggregationChange}>
        <Select.Trigger
          className={clsx(
            'inline-flex items-center justify-between',
            'px-3 py-2 min-w-[120px] rounded-lg',
            'bg-white dark:bg-gray-800',
            'border border-gray-300 dark:border-gray-700',
            'text-sm text-gray-900 dark:text-white',
            'hover:bg-gray-50 dark:hover:bg-gray-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          <Select.Value />
          <Select.Icon>
            <svg className="w-4 h-4 ml-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </Select.Icon>
        </Select.Trigger>

        <Select.Portal>
          <Select.Content
            className={clsx(
              'overflow-hidden rounded-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'shadow-lg'
            )}
          >
            <Select.Viewport className="p-1">
              {aggregationOptions.map((option) => (
                <Select.Item
                  key={option.value}
                  value={option.value}
                  className={clsx(
                    'relative flex items-center px-8 py-2 text-sm',
                    'text-gray-900 dark:text-white',
                    'rounded cursor-pointer',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                    'focus:outline-none focus:bg-gray-100 dark:focus:bg-gray-700',
                    'data-[state=checked]:bg-primary-50 dark:data-[state=checked]:bg-primary-900/30'
                  )}
                >
                  <Select.ItemText>{option.label}</Select.ItemText>
                  <Select.ItemIndicator className="absolute left-2">
                    <svg className="w-4 h-4 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                      <path
                        fillRule="evenodd"
                        d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                        clipRule="evenodd"
                      />
                    </svg>
                  </Select.ItemIndicator>
                </Select.Item>
              ))}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>

      {/* Custom Date Range Modal */}
      {showCustomRange && (
        <CustomDateRangeModal
          onSubmit={handleCustomRangeSubmit}
          onClose={() => setShowCustomRange(false)}
        />
      )}
    </div>
  );
}

interface CustomDateRangeModalProps {
  onSubmit: (start: Date, end: Date) => void;
  onClose: () => void;
}

function CustomDateRangeModal({ onSubmit, onClose }: CustomDateRangeModalProps) {
  const { t } = useTranslation('analytics');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (startDate && endDate) {
      onSubmit(new Date(startDate), new Date(endDate));
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />
      <div
        className={clsx(
          'relative z-10 p-6 rounded-xl',
          'bg-white dark:bg-gray-900',
          'border border-gray-200 dark:border-gray-800',
          'shadow-xl'
        )}
      >
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
          {t('customRange.title', 'Select Date Range')}
        </h3>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('customRange.startDate', 'Start Date')}
            </label>
            <input
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              className={clsx(
                'w-full px-3 py-2 rounded-lg',
                'bg-white dark:bg-gray-800',
                'border border-gray-300 dark:border-gray-700',
                'text-gray-900 dark:text-white',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
              required
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('customRange.endDate', 'End Date')}
            </label>
            <input
              type="date"
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
              className={clsx(
                'w-full px-3 py-2 rounded-lg',
                'bg-white dark:bg-gray-800',
                'border border-gray-300 dark:border-gray-700',
                'text-gray-900 dark:text-white',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
              required
            />
          </div>

          <div className="flex justify-end gap-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700'
              )}
            >
              {t('customRange.cancel', 'Cancel')}
            </button>
            <button
              type="submit"
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-primary-600 text-white',
                'hover:bg-primary-700'
              )}
            >
              {t('customRange.apply', 'Apply')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export default PeriodSelector;
