import { Fragment } from 'react';
import { Avatar } from '../../primitives/Avatar';
import type { PlayerView } from '../../types';
import styles from './badTakes.module.css';

const lowerFirst = (s: string) => s.replace(/\.$/, '').replace(/^./, (c) => c.toLowerCase());

export interface PromptCardProps {
  /** Prompt text with ____ for each blank. */
  text: string;
  /** Answers for the blanks, in order. A missing one leaves the blank showing. */
  answers?: (string | null | undefined)[];
  /** Shows 1 and 2 on the answers, for two-blank prompts. */
  numbered?: boolean;
  size?: 'normal' | 'big';
}
/** The prompt, with answers dropped into its blanks. */
export function PromptCard({ text, answers = [], numbered, size = 'normal' }: PromptCardProps) {
  const parts = text.split(/_{2,}/);
  return (
    <div className={`${styles.prompt} ${size === 'big' ? styles.promptBig : ''}`} data-prompt>
      {parts.map((part, i) => {
        const answer = answers[i];
        return (
          <Fragment key={i}>
            {part}
            {i < parts.length - 1 ? (answer
              ? <span className={styles.fill}>{numbered ? <span className={styles.num}>{i + 1}</span> : null}{lowerFirst(answer)}</span>
              : <span className={styles.blank} role="img" aria-label="blank" />) : null}
          </Fragment>
        );
      })}
    </div>
  );
}

export interface AnswerCardProps {
  text: string;
  selected?: boolean;
  /** 1 or 2 on a two-blank prompt. */
  order?: number;
  /** Leave out for a card people can only read. */
  onSelect?: () => void;
  wide?: boolean;
}
/** An answer card. */
export function AnswerCard({ text, selected, order, onSelect, wide }: AnswerCardProps) {
  const cls = `${styles.answer} ${selected ? styles.answerOn : ''} ${wide ? styles.answerWide : ''}`;
  if (!onSelect) return <div className={cls}>{text}</div>;
  return (
    <button type="button" className={`${cls} ${styles.tappable}`} aria-pressed={selected} onClick={onSelect}>
      {text}{order ? <span className={styles.order} aria-label={`Blank ${order}`}>{order}</span> : null}
    </button>
  );
}

export interface HandCard { id: string; text: string; }
export interface AnswerHandProps { cards: HandCard[]; /** Picked card ids, in order. */ picked: string[]; numbered?: boolean; onPick: (id: string) => void; }
/** Your seven cards, sized so they all show at once. */
export function AnswerHand({ cards, picked, numbered, onPick }: AnswerHandProps) {
  return (
    <div className={styles.hand}>
      {cards.map((c, i) => {
        const at = picked.indexOf(c.id);
        return <AnswerCard key={c.id} text={c.text} selected={at >= 0} order={numbered && at >= 0 ? at + 1 : undefined} wide={cards.length % 2 === 1 && i === cards.length - 1} onSelect={() => onPick(c.id)} />;
      })}
    </div>
  );
}

export interface PlayedStripProps { players: (PlayerView & { played: boolean })[]; }
/** Who has played. It never shows what they played. */
export function PlayedStrip({ players }: PlayedStripProps) {
  const done = players.filter((p) => p.played).length;
  return (
    <div className={styles.played}>
      {players.map((p) => <span key={p.id} data-player={p.id} className={styles.face}><Avatar nickname={p.nickname} kind={p.avatar} size="xs" dimmed={!p.played} /></span>)}
      <span>{done} of {players.length} have played</span>
    </div>
  );
}

export function RevealProgress({ total, shown }: { total: number; shown: number }) {
  return (
    <div className={styles.played} role="status">
      <span className={styles.pips} aria-hidden="true">{Array.from({ length: total }, (_, i) => <span key={i} className={`${styles.pip} ${i < shown ? styles.pipOut : ''}`} />)}</span>
      <span>Card {shown} of {total}</span>
    </div>
  );
}

export interface JudgeBadgeProps { judge: PlayerView; isMe?: boolean; }
export function JudgeBadge({ judge, isMe }: JudgeBadgeProps) {
  return <div className={styles.judge}><Avatar nickname={judge.nickname} kind={judge.avatar} size="sm" /><span>{isMe ? "You're the judge" : `${judge.nickname} is judging`}</span></div>;
}

export interface WinnerBannerProps { player: PlayerView; note?: string; }
/** Who won the round. The note covers "Time's up! Random pick". */
export function WinnerBanner({ player, note }: WinnerBannerProps) {
  return (
    <div className={styles.winner}>
      <Avatar nickname={player.nickname} kind={player.avatar} size="lg" highlight />
      <div><b className={styles.winnerName}>{player.nickname}</b><span>{note ?? 'wins the round'}</span></div>
    </div>
  );
}
