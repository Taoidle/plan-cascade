/**
 * useAutoSave Hook
 *
 * A reusable hook for auto-saving content with debouncing.
 * Prevents data loss while avoiding excessive writes.
 */

import { useEffect, useRef, useCallback } from 'react';

interface UseAutoSaveOptions {
  /** Content to auto-save */
  content: string;
  /** Callback to execute on save */
  onSave: () => Promise<void> | void;
  /** Debounce delay in milliseconds (default: 2000ms) */
  delay?: number;
  /** Whether auto-save is enabled (default: true) */
  enabled?: boolean;
  /** Minimum content length to trigger save (default: 0) */
  minLength?: number;
}

interface UseAutoSaveReturn {
  /** Whether a save is pending */
  isPending: boolean;
  /** Force an immediate save */
  saveNow: () => void;
  /** Cancel any pending save */
  cancel: () => void;
}

export function useAutoSave({
  content,
  onSave,
  delay = 2000,
  enabled = true,
  minLength = 0,
}: UseAutoSaveOptions): UseAutoSaveReturn {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastSavedContentRef = useRef<string>(content);
  const isPendingRef = useRef<boolean>(false);
  const isMountedRef = useRef<boolean>(true);

  // Clear timeout on unmount
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  // Execute save
  const executeSave = useCallback(async () => {
    if (!isMountedRef.current) return;

    try {
      await onSave();
      if (isMountedRef.current) {
        lastSavedContentRef.current = content;
        isPendingRef.current = false;
      }
    } catch (error) {
      console.error('Auto-save failed:', error);
      if (isMountedRef.current) {
        isPendingRef.current = false;
      }
    }
  }, [content, onSave]);

  // Force immediate save
  const saveNow = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }

    if (content !== lastSavedContentRef.current && content.length >= minLength) {
      executeSave();
    }
  }, [content, minLength, executeSave]);

  // Cancel pending save
  const cancel = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
      isPendingRef.current = false;
    }
  }, []);

  // Set up debounced auto-save
  useEffect(() => {
    if (!enabled) {
      cancel();
      return;
    }

    // Skip if content hasn't changed
    if (content === lastSavedContentRef.current) {
      return;
    }

    // Skip if content is too short
    if (content.length < minLength) {
      return;
    }

    // Clear existing timeout
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }

    // Set pending flag
    isPendingRef.current = true;

    // Set new timeout
    timeoutRef.current = setTimeout(() => {
      executeSave();
    }, delay);

    // Cleanup
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [content, enabled, delay, minLength, executeSave, cancel]);

  return {
    isPending: isPendingRef.current,
    saveNow,
    cancel,
  };
}

export default useAutoSave;
