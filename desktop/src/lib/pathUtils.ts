export function normalizeWorkspacePath(path: string | null | undefined): string | null {
  const value = (path || '').trim();
  if (!value) return null;

  let normalized = value.replace(/\\/g, '/');
  normalized = normalized.replace(/^\/\/\?\//, '');
  normalized = normalized.replace(/^\/\/\?\/unc\//i, '//');
  normalized = normalized.replace(/\/+$/, '');

  return normalized.toLowerCase();
}

export function workspacePathsEqual(left: string | null | undefined, right: string | null | undefined): boolean {
  const leftNormalized = normalizeWorkspacePath(left);
  const rightNormalized = normalizeWorkspacePath(right);
  return !!leftNormalized && !!rightNormalized && leftNormalized === rightNormalized;
}
