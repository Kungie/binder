/** The night task mark: a crescent in the night colour. */
export function Moon({ size = 12 }: { size?: number }) {
  return (
    <svg role="img" aria-label="Night task" width={size} height={size} viewBox="0 0 12 12" className="shrink-0 text-night">
      <path d="M8.6 7.9A4.2 4.2 0 0 1 4.1 3.4 4.2 4.2 0 0 1 4.6 1.5 4.6 4.6 0 1 0 10.5 7.4a4.2 4.2 0 0 1-1.9.5z" fill="currentColor" />
    </svg>
  );
}
