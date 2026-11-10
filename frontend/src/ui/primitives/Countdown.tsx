import { useEffect, useRef } from 'react';
import { TILTS } from './Tile';
import styles from './primitives.module.css';

export interface CountdownProps {
  /** Seconds left, from the server. */
  seconds: number;
  /** At this many seconds or fewer, the tiles turn to the accent colour. */
  lowAt?: number;
}

/** Each second the top tile drops away and the next one lands slightly crooked. */
export function Countdown({ seconds, lowAt = 10 }: CountdownProps) {
  const safe = Math.max(0, Math.min(99, Math.floor(seconds)));
  const digits = String(safe).padStart(2, '0');
  const box = useRef<HTMLDivElement>(null);
  const previous = useRef(digits);
  useEffect(() => {
    const el = box.current;
    const was = previous.current;
    previous.current = digits;
    if (!el || was === digits) return;
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    [...digits].forEach((digit, i) => {
      if (was[i] === digit) return;
      const tile = el.children[i] as HTMLElement;
      const ghost = tile.cloneNode(true) as HTMLElement;
      ghost.textContent = was[i];
      ghost.setAttribute('aria-hidden', 'true');
      ghost.style.position = 'absolute';
      ghost.style.left = `${tile.offsetLeft}px`;
      ghost.style.top = `${tile.offsetTop}px`;
      ghost.style.zIndex = '2';
      el.append(ghost);
      const side = i % 2 ? 1 : -1;
      ghost.animate(
        [{ transform: 'rotate(0deg)', opacity: 1 }, { transform: `translate(${side * 7}px, 34px) rotate(${side * 16}deg)`, opacity: 0 }],
        { duration: 230, easing: 'cubic-bezier(.5,0,1,.6)' },
      ).onfinish = () => ghost.remove();
      tile.animate([{ scale: 1.05 }, { scale: 1 }], { duration: 160, easing: 'ease-out' });
    });
  }, [digits]);
  return (
    <div ref={box} className={`${styles.countdown} ${safe <= lowAt ? styles.low : ''}`} role="timer" aria-label={`${safe} seconds left`}>
      {[...digits].map((d, i) => (
        <span key={i} aria-hidden="true" className={`${styles.tile} ${styles.timer}`} style={{ ['--tilt' as string]: `${TILTS[(safe + i) % TILTS.length]}deg` }}>{d}</span>
      ))}
    </div>
  );
}
