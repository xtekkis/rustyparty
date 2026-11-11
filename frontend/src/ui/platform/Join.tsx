import { useEffect, useRef, type ReactNode } from 'react';
import { Tile } from '../primitives/Tile';
import { Icon } from '../primitives/Icon';
import styles from './platform.module.css';

const LETTERS = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'];

export interface CodeSlotsProps {
  code: string;
  length?: number;
  /** True after a wrong code. The slots shake once each time it turns on. Added in rustyparty, from BRAND.md's motion table. */
  wrong?: boolean;
}
/** The empty slots a room code lands in, one tile at a time. */
export function CodeSlots({ code, length = 4, wrong }: CodeSlotsProps) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!wrong || !box.current) return;
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    box.current.animate(
      [{ translate: '0' }, { translate: '-8px' }, { translate: '7px' }, { translate: '-5px' }, { translate: '3px' }, { translate: '0' }],
      { duration: 240, easing: 'ease-out' },
    );
  }, [wrong]);
  return (
    <div ref={box} className={styles.slots} role="group" aria-label={`Room code, ${code.length} of ${length} letters`}>
      {Array.from({ length }, (_, i) => code[i]
        ? <Tile key={i} size="slot" tilt={i}>{code[i]}</Tile>
        : <span key={i} className={`${styles.emptySlot} ${i === code.length ? styles.nextSlot : ''}`} />)}
    </div>
  );
}

export interface LetterTrayProps { onLetter: (letter: string) => void; onDelete: () => void; disabled?: boolean; }
/** The A to Z tiles that replace the phone keyboard on the join screen. */
export function LetterTray({ onLetter, onDelete, disabled }: LetterTrayProps) {
  return (
    <div className={styles.tray}>
      {LETTERS.map((l) => <button key={l} type="button" className={styles.key} disabled={disabled} onClick={() => onLetter(l)}>{l}</button>)}
      <button type="button" className={`${styles.key} ${styles.deleteKey}`} aria-label="Delete last letter" onClick={onDelete}><Icon name="delete" /></button>
    </div>
  );
}

/** The camera frame. Pass the video element as a child. */
export function ScannerFrame({ children }: { children?: ReactNode }) {
  return <div className={styles.scanner} role="img" aria-label="Live camera view">{children}<i /><i /><i /><i /></div>;
}
