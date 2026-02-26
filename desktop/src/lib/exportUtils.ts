/**
 * Export utilities for StreamingOutput component.
 *
 * Provides text/binary save helpers, Markdown serialization,
 * DOM screenshot capture (PNG/JPG), and PDF generation.
 */

import { invoke } from '@tauri-apps/api/core';
import type { StreamLine } from '../store/execution';

// ============================================================================
// Types
// ============================================================================

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

// ============================================================================
// Filename helpers
// ============================================================================

export function localTimestampForFilename(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}T${pad(now.getHours())}-${pad(now.getMinutes())}-${pad(now.getSeconds())}`;
}

// ============================================================================
// Save helpers
// ============================================================================

export async function saveTextWithDialog(filename: string, content: string): Promise<boolean> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const selected = await save({
    title: 'Export Output',
    defaultPath: filename,
    canCreateDirectories: true,
  });
  if (!selected || Array.isArray(selected)) return false;
  const result = await invoke<CommandResponse<boolean>>('save_output_export', {
    path: selected,
    content,
  });
  if (!result.success) {
    throw new Error(result.error || 'Failed to save export');
  }
  return true;
}

export async function saveBinaryWithDialog(filename: string, dataBase64: string): Promise<boolean> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const selected = await save({
    title: 'Export Output',
    defaultPath: filename,
    canCreateDirectories: true,
  });
  if (!selected || Array.isArray(selected)) return false;
  const result = await invoke<CommandResponse<boolean>>('save_binary_export', {
    path: selected,
    dataBase64,
  });
  if (!result.success) {
    throw new Error(result.error || 'Failed to save export');
  }
  return true;
}

// ============================================================================
// Text serialization
// ============================================================================

const LINE_TYPE_PREFIX: Record<string, string> = {
  text: '',
  info: 'INFO ',
  error: 'ERR  ',
  success: 'OK   ',
  warning: 'WARN ',
  tool: '',
  tool_result: '',
  sub_agent: '',
  analysis: '',
  thinking: '     ',
  code: '',
  card: '',
};

export function serializeRawOutput(lines: StreamLine[]): string {
  return lines
    .map((line) => {
      const prefix = LINE_TYPE_PREFIX[line.type] ?? '';
      if (line.type === 'text') return line.content;
      return `${prefix}${line.content}`;
    })
    .join('\n');
}

export function serializeConversationOutput(lines: StreamLine[]): string {
  const out: string[] = [];
  for (const line of lines) {
    const content = line.content.trim();
    if (!content) continue;
    switch (line.type) {
      case 'info':
        out.push(`User: ${content}`);
        break;
      case 'text':
        out.push(`Assistant: ${content}`);
        break;
      case 'error':
        out.push(`Error: ${content}`);
        break;
      case 'warning':
        out.push(`Warning: ${content}`);
        break;
      case 'success':
        out.push(`Status: ${content}`);
        break;
      default:
        break;
    }
  }
  return out.join('\n\n');
}

export function collectAssistantReplies(lines: StreamLine[]): Array<{ id: number; content: string }> {
  return lines
    .filter((line) => line.type === 'text' && line.content.trim().length > 0)
    .map((line) => ({ id: line.id, content: line.content }));
}

export function serializeConversationMarkdown(lines: StreamLine[]): string {
  const parts: string[] = [];
  for (const line of lines) {
    const content = line.content.trim();
    if (!content) continue;
    switch (line.type) {
      case 'info':
        parts.push(`> **User**\n>\n> ${content.replace(/\n/g, '\n> ')}`);
        break;
      case 'text':
        parts.push(`**Assistant**\n\n${content}`);
        break;
      case 'error':
        parts.push(`> **Error**: ${content.replace(/\n/g, '\n> ')}`);
        break;
      case 'warning':
        parts.push(`> **Warning**: ${content.replace(/\n/g, '\n> ')}`);
        break;
      case 'success':
        parts.push(`> **Status**: ${content.replace(/\n/g, '\n> ')}`);
        break;
      case 'tool':
        parts.push(`\`\`\`\n[Tool] ${content}\n\`\`\``);
        break;
      case 'tool_result':
        parts.push(`\`\`\`\n[Result] ${content}\n\`\`\``);
        break;
      case 'thinking':
        parts.push(`<details>\n<summary>Thinking</summary>\n\n${content}\n\n</details>`);
        break;
      default:
        break;
    }
  }
  return parts.join('\n\n---\n\n');
}

// ============================================================================
// DOM capture helpers
// ============================================================================

/**
 * Temporarily expand a scroll container to its full scroll dimensions so that
 * `html-to-image` can capture everything (not just the visible viewport).
 * Returns a restore function that puts everything back.
 */
function expandForCapture(element: HTMLElement): () => void {
  const saved = {
    overflow: element.style.overflow,
    maxHeight: element.style.maxHeight,
    height: element.style.height,
    flex: element.style.flex,
    minHeight: element.style.minHeight,
  };

  const fullHeight = element.scrollHeight;
  const fullWidth = element.scrollWidth;

  element.style.overflow = 'visible';
  element.style.maxHeight = 'none';
  element.style.height = `${fullHeight}px`;
  element.style.flex = 'none';
  element.style.minHeight = 'auto';

  return () => {
    element.style.overflow = saved.overflow;
    element.style.maxHeight = saved.maxHeight;
    element.style.height = saved.height;
    element.style.flex = saved.flex;
    element.style.minHeight = saved.minHeight;
    // Force layout reflow so browser re-applies the scroll container
    void element.offsetHeight;
    // Ignore the unused value — we only need the side-effect (layout reflow)
    void fullWidth;
  };
}

const nodeFilter = (node: HTMLElement) => {
  if (node.dataset && node.dataset.exportExclude === 'true') return false;
  return true;
};

export async function captureElementToBlob(
  element: HTMLElement,
  format: 'png' | 'jpeg',
  options?: { backgroundColor?: string },
): Promise<Blob> {
  const htmlToImage = await import('html-to-image');

  const restore = expandForCapture(element);
  try {
    // Measure after expanding
    const w = element.scrollWidth;
    const h = element.scrollHeight;
    let pixelRatio = 2;
    if (w * h * 4 * pixelRatio * pixelRatio > 200_000_000) {
      pixelRatio = 1;
    }

    const opts = {
      pixelRatio,
      backgroundColor: options?.backgroundColor ?? '#030712',
      filter: nodeFilter,
      width: w,
      height: h,
    };

    const dataUrl =
      format === 'png'
        ? await htmlToImage.toPng(element, opts)
        : await htmlToImage.toJpeg(element, { ...opts, quality: 0.92 });

    const res = await fetch(dataUrl);
    return res.blob();
  } finally {
    restore();
  }
}

export async function captureElementToPdfBlob(element: HTMLElement): Promise<Blob> {
  const htmlToImage = await import('html-to-image');
  const { jsPDF } = await import('jspdf');

  const restore = expandForCapture(element);
  try {
    const w = element.scrollWidth;
    const h = element.scrollHeight;
    let pixelRatio = 2;
    if (w * h * 4 * pixelRatio * pixelRatio > 200_000_000) {
      pixelRatio = 1;
    }

    const dataUrl = await htmlToImage.toPng(element, {
      pixelRatio,
      backgroundColor: '#030712',
      filter: nodeFilter,
      width: w,
      height: h,
    });

    // Create canvas from dataUrl to slice into pages
    const img = new Image();
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = reject;
      img.src = dataUrl;
    });

    // A4 dimensions in mm
    const A4_WIDTH_MM = 210;
    const A4_HEIGHT_MM = 297;
    const MARGIN_MM = 10;
    const contentWidthMm = A4_WIDTH_MM - 2 * MARGIN_MM;
    const contentHeightMm = A4_HEIGHT_MM - 2 * MARGIN_MM;

    // Scale image to fit A4 width
    const scale = contentWidthMm / (w * pixelRatio);
    const scaledHeightMm = h * pixelRatio * scale;

    const pdf = new jsPDF({ orientation: 'portrait', unit: 'mm', format: 'a4' });

    if (scaledHeightMm <= contentHeightMm) {
      // Single page
      pdf.addImage(dataUrl, 'PNG', MARGIN_MM, MARGIN_MM, contentWidthMm, scaledHeightMm);
    } else {
      // Multi-page: slice the image into page-sized chunks
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d')!;
      const imgW = img.width;
      const imgH = img.height;

      // How many image pixels fit in one page height
      const pageImgHeight = Math.floor(contentHeightMm / scale);

      let yOffset = 0;
      let pageIndex = 0;
      while (yOffset < imgH) {
        const sliceH = Math.min(pageImgHeight, imgH - yOffset);
        canvas.width = imgW;
        canvas.height = sliceH;
        ctx.fillStyle = '#030712';
        ctx.fillRect(0, 0, imgW, sliceH);
        ctx.drawImage(img, 0, yOffset, imgW, sliceH, 0, 0, imgW, sliceH);

        const sliceDataUrl = canvas.toDataURL('image/png');
        const sliceHeightMm = sliceH * scale;

        if (pageIndex > 0) pdf.addPage();
        pdf.addImage(sliceDataUrl, 'PNG', MARGIN_MM, MARGIN_MM, contentWidthMm, sliceHeightMm);

        yOffset += sliceH;
        pageIndex++;
      }
    }

    return pdf.output('blob');
  } finally {
    restore();
  }
}

// ============================================================================
// Blob helpers
// ============================================================================

export function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onloadend = () => {
      const result = reader.result as string;
      // Strip the data:...;base64, prefix
      const idx = result.indexOf(',');
      resolve(idx >= 0 ? result.substring(idx + 1) : result);
    };
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}
