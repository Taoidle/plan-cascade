import { useCallback, useState, type RefObject } from 'react';
import type { TFunction } from 'i18next';
import {
  captureElementToBlob,
  blobToBase64,
  saveBinaryWithDialog,
  localTimestampForFilename,
} from '../../lib/exportUtils';

type ToastLevel = 'info' | 'success' | 'error';

interface UseSimpleExportParams {
  chatScrollRef: RefObject<HTMLDivElement | null>;
  showToast: (message: string, level?: ToastLevel) => void;
  t: TFunction<'simpleMode'>;
}

export function useSimpleExport({ chatScrollRef, showToast, t }: UseSimpleExportParams) {
  const [isCapturing, setIsCapturing] = useState(false);

  const handleExportImage = useCallback(async () => {
    const el = chatScrollRef.current;
    if (!el) return;

    const previousScrollTop = el.scrollTop;
    const waitForNextFrame = () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => resolve());
      });

    setIsCapturing(true);
    try {
      await waitForNextFrame();
      await waitForNextFrame();
      const isDark = document.documentElement.classList.contains('dark');
      const blob = await captureElementToBlob(el, 'png', {
        backgroundColor: isDark ? '#111827' : '#ffffff',
      });
      const base64 = await blobToBase64(blob);
      const ts = localTimestampForFilename();
      const saved = await saveBinaryWithDialog(`chat-export-${ts}.png`, base64);
      if (saved) {
        showToast(t('chatToolbar.exportImageSuccess', { defaultValue: 'Image exported successfully' }), 'success');
      }
    } catch (err) {
      console.error('Export image failed:', err);
      showToast(t('chatToolbar.exportImageFailed', { defaultValue: 'Failed to export image' }), 'error');
    } finally {
      setIsCapturing(false);
      requestAnimationFrame(() => {
        if (chatScrollRef.current) {
          chatScrollRef.current.scrollTop = previousScrollTop;
        }
      });
    }
  }, [chatScrollRef, showToast, t]);

  return {
    isCapturing,
    handleExportImage,
  };
}
