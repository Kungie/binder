/** Load average over the cores, rounded: 0.74 on 2 cores is "37%". Above 100% shows as is, since work is waiting.
 * Null without a core count. */
export function loadPercent(load: number, cores: number): string | null {
  return cores > 0 ? `${Math.round((load / cores) * 100)}%` : null;
}
