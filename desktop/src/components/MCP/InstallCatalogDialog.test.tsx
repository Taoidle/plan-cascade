import { describe, expect, it } from 'vitest';
import { shouldDisableInstallButton } from './InstallCatalogDialog';

describe('InstallCatalogDialog gating logic', () => {
  it('blocks installation when preview is not ready or has errors', () => {
    expect(
      shouldDisableInstallButton({
        installing: false,
        hasItem: true,
        previewLoading: true,
        hasPreview: false,
        previewError: null,
        communityConfirmed: true,
        hasMissingRequiredSecrets: false,
      }),
    ).toBe(true);

    expect(
      shouldDisableInstallButton({
        installing: false,
        hasItem: true,
        previewLoading: false,
        hasPreview: true,
        previewError: 'preview failed',
        communityConfirmed: true,
        hasMissingRequiredSecrets: false,
      }),
    ).toBe(true);
  });

  it('allows installation only when all gating conditions are satisfied', () => {
    expect(
      shouldDisableInstallButton({
        installing: false,
        hasItem: true,
        previewLoading: false,
        hasPreview: true,
        previewError: null,
        communityConfirmed: true,
        hasMissingRequiredSecrets: false,
      }),
    ).toBe(false);

    expect(
      shouldDisableInstallButton({
        installing: false,
        hasItem: true,
        previewLoading: false,
        hasPreview: true,
        previewError: null,
        communityConfirmed: false,
        hasMissingRequiredSecrets: false,
      }),
    ).toBe(true);

    expect(
      shouldDisableInstallButton({
        installing: false,
        hasItem: true,
        previewLoading: false,
        hasPreview: true,
        previewError: null,
        communityConfirmed: true,
        hasMissingRequiredSecrets: true,
      }),
    ).toBe(true);
  });
});
