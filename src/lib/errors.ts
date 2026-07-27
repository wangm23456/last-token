export function getErrorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  if (isRecord(err) && hasString(err, "message")) {
    return err.message;
  }
  return String(err);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

function hasString(
  record: Record<string, unknown>,
  key: string,
): record is Record<string, unknown> & { [k in typeof key]: string } {
  return typeof record[key] === "string";
}
