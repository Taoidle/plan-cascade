export interface ActionResult {
  ok: boolean;
  errorCode?: string | null;
  message?: string | null;
}

export function okResult(message?: string | null): ActionResult {
  return { ok: true, errorCode: null, message: message ?? null };
}

export function failResult(errorCode: string, message: string): ActionResult {
  return { ok: false, errorCode, message };
}
