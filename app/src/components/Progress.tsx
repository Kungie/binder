/** A thin bar: the filled part is text colour, the rest the line colour. */
export function Progress({ value, label }: { value: number; label: string }) {
  const v = Math.max(0, Math.min(100, value));
  return (
    <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={v} className="h-1 w-full overflow-hidden rounded-full bg-line-strong">
      <div className="h-full bg-text transition-[width] duration-300" style={{ width: `${v}%` }} />
    </div>
  );
}
