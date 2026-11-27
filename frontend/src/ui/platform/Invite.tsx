import { useEffect, type ReactNode } from 'react';
import { ActionTile, Button, TopBar } from '../primitives/Controls';
import { Screen } from '../primitives/Screen';
import { RoomCode } from '../primitives/Tile';
import type { World } from '../types';
import styles from './platform.module.css';

/** Added in rustyparty: a compact Invite tile for a game's top bar. */
export function InviteTile({ onClick }: { onClick: () => void }) {
  return <ActionTile icon="scan" label="Invite" aria-label="Invite players" className={styles.inviteTile} onClick={onClick} />;
}

export interface InviteCardProps {
  world?: World;
  code: string;
  qr: ReactNode;
  onCopy: () => void;
  onShare?: () => void;
  onClose: () => void;
}
/** Added in rustyparty: the room code and a big QR code over the whole screen, so more people can join. */
export function InviteCard({ world = 'platform', code, qr, onCopy, onShare, onClose }: InviteCardProps) {
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') onClose(); };
    window.addEventListener('keydown', escape);
    return () => window.removeEventListener('keydown', escape);
  }, [onClose]);
  return (
    <div className={styles.inviteCard} role="dialog" aria-modal="true" aria-label="Invite players">
      <Screen world={world}
        glance={<><TopBar>Invite players</TopBar><div className={styles.inviteCode}><RoomCode code={code} /></div><div className={styles.bigQr}>{qr}</div></>}
        thumb={<div className={`${styles.readyRow} ${onShare ? '' : styles.noShare}`}>
          <Button autoFocus onClick={onClose}>Close</Button>
          <ActionTile icon="copy" label="Copy" aria-label="Copy invite link" onClick={onCopy} />
          {onShare ? <ActionTile icon="share" label="Share" aria-label="Share invite link" onClick={onShare} /> : null}
        </div>} />
    </div>
  );
}