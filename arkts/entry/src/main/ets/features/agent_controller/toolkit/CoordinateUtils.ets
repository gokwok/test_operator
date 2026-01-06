export function normToPx(limit: number, value: number): number {
  const safe = Number.isFinite(value) ? value : 0;
  const clamped = Math.min(Math.max(safe, 0), 1000);
  const max = Math.max(1, limit);
  const px = Math.round((clamped / 1000) * (max - 1));
  return px < 0 ? 0 : px;
}
