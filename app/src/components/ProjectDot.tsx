/** A project's colour: a small dot beside its name. */
export function ProjectDot({ color, size = 6 }: { color: string; size?: number }) {
  return <span data-color={color} aria-hidden="true" className="inline-block shrink-0 rounded-full" style={{ width: size, height: size, background: color }} />;
}
