import { Avatar } from '../primitives/Avatar';
import type { PlayerView, ScoreRow } from '../types';
import styles from './platform.module.css';

/** Everyone's score, highest first. A point just won shows +1. */
export function Scoreboard({ rows }: { rows: ScoreRow[] }) {
  const sorted = [...rows].sort((a, b) => b.score - a.score);
  return (
    <div className={styles.score} role="list" aria-label="Scores">
      {sorted.map((r) => (
        <div key={r.player.id} role="listitem" className={styles.scoreRow}>
          <Avatar nickname={r.player.nickname} kind={r.player.avatar} size="xs" />
          <span>{r.player.nickname}</span>
          {r.gained ? <span className={styles.plus}>+1</span> : null}
          <span className={styles.points_n}>{r.score}</span>
        </div>
      ))}
    </div>
  );
}

export interface ChampionProps { player?: PlayerView; title: string; }
/** The big winner at the end of a game, or just a title when nobody won. */
export function Champion({ player, title }: ChampionProps) {
  return (
    <div className={styles.champ}>
      {player ? <Avatar nickname={player.nickname} kind={player.avatar} size="xl" highlight /> : null}
      <h1 className={styles.champTitle}>{title}</h1>
    </div>
  );
}
