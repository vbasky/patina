/** Assert a value is non-null/undefined, returning it narrowed. Throws with a
 *  clear message otherwise — used where a value is invariably present (e.g. a
 *  reducer acting on a notebook that must exist). */
export function nonNull<T>(
  value: T | null | undefined,
  msg = "unexpected null",
): T {
  if (value == null) {
    throw new Error(msg);
  }
  return value;
}
