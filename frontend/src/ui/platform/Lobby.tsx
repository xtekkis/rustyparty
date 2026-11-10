import { useState, type ReactNode } from 'react';
import { Avatar } from '../primitives/Avatar';
import { ActionTile, Button } from '../primitives/Controls';
import { Icon } from '../primitives/Icon';
import type { PlayerView } from '../types';
import styles from './platform.module.css';

export interface PlayerListProps {
  players: PlayerView[];
  /** Host only: shows Kick and Ban on every other player. */
  moderatorId?: string;
  onKick?: (id: string) => void;
  /** Runs after the host confirms. */
  onBan?: (id: string) => void;
}

/** Everyone in the room. It scrolls once the room fills up. Ban asks once before it runs. */
export function PlayerList({ players, moderatorId, onKick, onBan }: PlayerListProps) {
  const [confirming, setConfirming] = useState<string | null>(null);
  return (
    <div className={styles.list} tabIndex={0} aria-label="Players">
      {players.map((p) => {
        const offline = p.connected === false;
        const canModerate = moderatorId !== undefined && p.id !== moderatorId;
        return (
          <div key={p.id} className={styles.row}>
            <Avatar nickname={p.nickname} kind={p.avatar} dimmed={offline} />
            <div className={styles.name}>
              <span className={offline ? styles.fadedName : ''}>{p.nickname}</span>
              {p.isHost ? <span className={styles.host}>Host</span> : null}
              {p.ready ? <span className={styles.tick} role="img" aria-label="Ready"><Icon name="tick" /></span> : null}
              {offline ? <small className={styles.nameNote}>Reconnecting{p.reconnectSeconds != null ? `, ${p.reconnectSeconds}s` : ''}</small> : null}
              {confirming === p.id ? <small className={styles.nameNote}>Can't rejoin</small> : null}
            </div>
            {canModerate ? (
              <div className={styles.mods}>
                {confirming === p.id ? (
                  <>
                    <button type="button" className={`${styles.mod} ${styles.modYes}`} aria-label={`Yes, ban ${p.nickname}`} onClick={() => { setConfirming(null); onBan?.(p.id); }}>Ban</button>
                    <button type="button" className={styles.mod} aria-label={`Keep ${p.nickname}`} onClick={() => setConfirming(null)}>Keep</button>
                  </>
                ) : (
                  <>
                    <button type="button" className={styles.mod} aria-label={`Kick ${p.nickname}`} onClick={() => onKick?.(p.id)}>Kick</button>
                    <button type="button" className={styles.mod} aria-label={`Ban ${p.nickname}`} onClick={() => setConfirming(p.id)}>Ban</button>
                  </>
                )}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

export interface PointsStepperProps { value: number; min?: number; max?: number; onChange: (value: number) => void; }
/** The host's points-to-win setting, 3 to 15. */
export function PointsStepper({ value, min = 3, max = 15, onChange }: PointsStepperProps) {
  return (
    <div className={styles.points}>
      <span>Points to win</span>
      <span className={styles.step}>
        <button type="button" aria-label="Fewer points" disabled={value <= min} onClick={() => onChange(value - 1)}>&minus;</button>
        <b aria-live="polite">{value}</b>
        <button type="button" aria-label="More points" disabled={value >= max} onClick={() => onChange(value + 1)}>+</button>
      </span>
    </div>
  );
}

export interface ReadyRowProps {
  ready: boolean;
  onToggleReady: () => void;
  onCopy: () => void;
  /** Leave out on phones without a share menu. Ready grows to fill the space. */
  onShare?: () => void;
}
/** Ready with the invite actions beside it, in thumb reach. */
export function ReadyRow({ ready, onToggleReady, onCopy, onShare }: ReadyRowProps) {
  return (
    <div className={`${styles.readyRow} ${onShare ? '' : styles.noShare}`}>
      <Button variant={ready ? 'success' : 'primary'} aria-pressed={ready} sublabel={ready ? 'Tap to undo' : undefined} onClick={onToggleReady}>Ready</Button>
      <ActionTile icon="copy" label="Copy" aria-label="Copy invite link" onClick={onCopy} />
      {onShare ? <ActionTile icon="share" label="Share" aria-label="Share invite link" onClick={onShare} /> : null}
    </div>
  );
}

/** The white square a QR code needs. Pass the QR as an svg child. */
export function QrFrame({ children }: { children: ReactNode }) {
  return <div className={styles.qr}>{children}</div>;
}
