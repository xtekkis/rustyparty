import type { ReactNode } from 'react';
import styles from './primitives.module.css';

export interface StatusBannerProps { children: ReactNode; tone?: 'info' | 'alert'; }

/** A short banner over any screen: Reconnecting, a server error, or a round message. The app decides how long it stays. */
export function StatusBanner({ children, tone = 'info' }: StatusBannerProps) {
  return <div role={tone === 'alert' ? 'alert' : 'status'} className={`${styles.banner} ${tone === 'alert' ? styles.alert : ''}`}>{children}</div>;
}
