export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 KB";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Positive means the output got smaller than the input. */
export function savingsPercent(original: number, output: number): number {
  if (original <= 0) return 0;
  return Math.round((1 - output / original) * 100);
}
