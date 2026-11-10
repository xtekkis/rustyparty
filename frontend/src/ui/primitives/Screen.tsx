import type { ReactNode } from 'react';
import { Halftone } from './Halftone';
import type { World } from '../types';
import styles from './primitives.module.css';

export interface ScreenProps {
  /** Which world paints this screen. */
  world?: World;
  /** Things people read: titles, codes, prompts, timers. */
  glance: ReactNode;
  /** Things people tap. Anchored to the bottom in portrait and to the right when sideways. */
  thumb?: ReactNode;
  /** Centres the glance zone vertically, for Home and message screens. */
  centerGlance?: boolean;
  /** Overlays such as StatusBanner. */
  children?: ReactNode;
}

/** The shell of every screen: a glance zone, a thumb zone and the dots behind them. */
export function Screen({ world = 'platform', glance, thumb, centerGlance, children }: ScreenProps) {
  return (
    <div data-theme={world} className={styles.screen}>
      <Halftone world={world} />
      <div className={styles.layout}>
        <div className={`${styles.glance} ${centerGlance ? styles.center : ''}`}>{glance}</div>
        {thumb ? <div className={styles.thumb}>{thumb}</div> : null}
      </div>
      {children}
    </div>
  );
}
