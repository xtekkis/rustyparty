export type IconName = 'back' | 'copy' | 'share' | 'delete' | 'scan' | 'tick';
const PATHS: Record<IconName, { box: string; d: string }> = {
  back: { box: '0 0 24 24', d: 'M10 5l-7 7 7 7M3 12h18' },
  copy: { box: '0 0 24 24', d: 'M9 9h11v11H9zM5 15H4V4h11v1' },
  share: { box: '0 0 24 24', d: 'M12 15V3M7 8l5-5 5 5M5 13v8h14v-8' },
  delete: { box: '0 0 28 24', d: 'M9 3h17v18H9l-7-9zM13 8l8 8M21 8l-8 8' },
  scan: { box: '0 0 24 24', d: 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5M9 9h6v6H9z' },
  tick: { box: '0 0 20 20', d: 'M4 11l4 4 8-10' },
};
/** Square-ended line icons. They take the text colour of their parent. */
export function Icon({ name, className }: { name: IconName; className?: string }) {
  const { box, d } = PATHS[name];
  return (
    <svg viewBox={box} width="24" height="24" fill="none" stroke="currentColor" strokeWidth={name === 'tick' ? 3 : 2} strokeLinecap="square" aria-hidden="true" className={className}>
      <path d={d} />
    </svg>
  );
}
