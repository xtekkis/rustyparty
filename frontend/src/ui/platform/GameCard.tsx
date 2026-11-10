import { Avatar } from '../primitives/Avatar';
import type { GameOption } from '../types';
import styles from './platform.module.css';

export interface GameCardProps { game: GameOption; selected?: boolean; onSelect?: () => void; }

/** A game on the vote screen, painted in that game's own world. Voters appear on it live. */
export function GameCard({ game, selected, onSelect }: GameCardProps) {
  if (game.unavailableReason) {
    return (
      <button type="button" className={`${styles.game} ${styles.gameOff}`} disabled aria-disabled="true">
        <span className={styles.gameOffName}>{game.name}: {game.unavailableReason}</span>
      </button>
    );
  }
  return (
    <button type="button" data-theme={game.world} aria-pressed={selected} onClick={onSelect} className={`${styles.game} ${game.world ? styles.gameWorld : styles.gamePlain} ${selected ? styles.gameOn : ''}`}>
      <span className={styles.gameName}>{game.name}</span>
      <span className={styles.gameText}>{game.description}</span>
      <span className={styles.voters}>{game.voters.map((v) => <Avatar key={v.id} nickname={v.nickname} kind={v.avatar} size="xs" />)}</span>
    </button>
  );
}
