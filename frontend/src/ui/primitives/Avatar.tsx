import type { ReactElement } from 'react';
import type { AvatarKind } from '../types';
import styles from './primitives.module.css';

const FACES: Record<AvatarKind, ReactElement> = {
  fox: <><path className={styles.m} d="M6 7l8 8h12l8-8-1 18-13 11L7 25z" /><path className={styles.l} d="M12 25l8 11 8-11-8 3z" /><circle className={styles.d} cx="14.5" cy="20" r="2" /><circle className={styles.d} cx="25.5" cy="20" r="2" /></>,
  cat: <><path className={styles.m} d="M8 6l8 7h8l8-7 1 18a13 12 0 0 1-26 0z" /><circle className={styles.l} cx="14.5" cy="21" r="2.4" /><circle className={styles.l} cx="25.5" cy="21" r="2.4" /><path className={styles.l} d="M18 26h4l-2 2.5z" /></>,
  owl: <><path className={styles.m} d="M8 10q12-8 24 0v16q0 10-12 10T8 26z" /><circle className={styles.l} cx="14.5" cy="18" r="5.5" /><circle className={styles.l} cx="25.5" cy="18" r="5.5" /><circle className={styles.d} cx="14.5" cy="18" r="2.2" /><circle className={styles.d} cx="25.5" cy="18" r="2.2" /><path className={styles.l} d="M18 24h4l-2 3.5z" /></>,
  bear: <><circle className={styles.m} cx="10" cy="11" r="5.5" /><circle className={styles.m} cx="30" cy="11" r="5.5" /><circle className={styles.m} cx="20" cy="22" r="14" /><ellipse className={styles.l} cx="20" cy="27" rx="6.5" ry="5" /><circle className={styles.l} cx="14.5" cy="19" r="2.2" /><circle className={styles.l} cx="25.5" cy="19" r="2.2" /><circle className={styles.d} cx="20" cy="25.5" r="1.8" /></>,
  frog: <><circle className={styles.m} cx="12" cy="11" r="6.5" /><circle className={styles.m} cx="28" cy="11" r="6.5" /><ellipse className={styles.m} cx="20" cy="24" rx="16" ry="12" /><circle className={styles.l} cx="12" cy="11" r="3.6" /><circle className={styles.l} cx="28" cy="11" r="3.6" /><circle className={styles.d} cx="12" cy="11" r="1.7" /><circle className={styles.d} cx="28" cy="11" r="1.7" /><path d="M11 27q9 6 18 0" fill="none" stroke="var(--avatar-feature)" strokeWidth="2.4" /></>,
};

/** The same nickname always gets the same colour. */
export function initialColor(nickname: string) {
  let hash = 0;
  for (const c of nickname) hash = (hash * 31 + (c.codePointAt(0) ?? 0)) % 1009;
  return `var(--initial-${(hash % 5) + 1})`;
}

export interface AvatarProps {
  nickname: string;
  kind?: AvatarKind | null;
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  /** Waiting or not played yet. */
  dimmed?: boolean;
  /** The accent ring, for winners. */
  highlight?: boolean;
}

/** A player's picture, or their initial on a colour from their nickname. */
export function Avatar({ nickname, kind, size = 'md', dimmed, highlight }: AvatarProps) {
  const cls = `${styles.avatar} ${styles['av_' + size]} ${dimmed ? styles.dimmed : ''} ${highlight ? styles.highlight : ''}`;
  if (!kind) {
    return <span className={`${cls} ${styles.initial}`} style={{ background: initialColor(nickname) }} aria-hidden="true">{[...nickname][0]?.toUpperCase()}</span>;
  }
  return <span className={`${cls} ${styles['av_' + kind]}`} aria-hidden="true"><svg viewBox="0 0 40 40">{FACES[kind]}</svg></span>;
}

export const AVATAR_KINDS: AvatarKind[] = ['fox', 'cat', 'owl', 'bear', 'frog'];
