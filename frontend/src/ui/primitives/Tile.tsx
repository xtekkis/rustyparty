import type { ReactNode } from 'react';
import styles from './primitives.module.css';

export const TILTS = [-2.4, 1.8, -1.2, 2.2, -1.8, 1.4, -2.6, 1];
export interface TileProps {
  children: ReactNode;
  /** Position in the word. It picks the slight crooked angle. Omit for a straight tile. */
  tilt?: number;
  tone?: 'default' | 'accent';
  size?: 'logo' | 'code' | 'slot' | 'timer';
}

/** The letter tile: the shape rustyparty is known by. */
export function Tile({ children, tilt, tone = 'default', size = 'code' }: TileProps) {
  const angle = tilt === undefined ? 0 : TILTS[tilt % TILTS.length];
  return (
    <span className={`${styles.tile} ${styles[size]} ${tone === 'accent' ? styles.accentTile : ''}`} style={{ ['--tilt' as string]: `${angle}deg` }}>
      {children}
    </span>
  );
}

/** The logo. Set in tiles, so there is no separate image file. */
export function Wordmark() {
  return (
    <div className={styles.wordmark} role="img" aria-label="rustyparty">
      <div className={styles.line} aria-hidden="true">{[...'RUSTY'].map((l, i) => <Tile key={i} size="logo" tilt={i}>{l}</Tile>)}</div>
      <div className={styles.line} aria-hidden="true">{[...'PARTY'].map((l, i) => <Tile key={i} size="logo" tilt={i + 3} tone={i === 0 ? 'accent' : 'default'}>{l}</Tile>)}</div>
    </div>
  );
}

/** The room code, big enough to read across a table. */
export function RoomCode({ code }: { code: string }) {
  return (
    <div className={styles.roomCode} role="img" aria-label={`Room code ${code}`}>
      {[...code].map((l, i) => <span key={i} aria-hidden="true"><Tile tilt={i}>{l}</Tile></span>)}
    </div>
  );
}
