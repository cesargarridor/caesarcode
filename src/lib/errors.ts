import type { AppError } from './types';

/** Convierte cualquier error de `invoke` (AppError serializado, Error, string) a texto. */
export function toMessage(err: unknown): string {
  if (err && typeof err === 'object' && 'message' in err) {
    const m = (err as Partial<AppError>).message;
    if (typeof m === 'string') return m;
  }
  return String(err);
}
