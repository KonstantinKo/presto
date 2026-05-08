/**
 * @param {unknown} value
 * @returns {Error}
 */
export function toError(value) {
  return value instanceof Error ? value : new Error(String(value));
}
