import { forwardRef } from 'react';
import { Screen } from '../../primitives/Screen';
import { Button, Strip, TopBar } from '../../primitives/Controls';
import { Countdown } from '../../primitives/Countdown';
import { Scoreboard } from '../../platform/Scoreboard';
import { AnswerCard, AnswerHand, JudgeBadge, PlayedStrip, PromptCard, RevealProgress, WinnerBanner, type HandCard } from './Cards';
import { CardBag, type CardBagHandle } from './CardBag';
import type { PlayerView, ScoreRow } from '../../types';
import styles from './badTakes.module.css';

const WORLD = 'bad-takes' as const;

export interface BadTakesPlayScreenProps {
  judge: PlayerView;
  seconds: number;
  prompt: string;
  blanks: 1 | 2;
  hand: HandCard[];
  /** Picked card ids, in order. */
  picked: string[];
  others: (PlayerView & { played: boolean })[];
  onPick: (id: string) => void;
}
export function BadTakesPlayScreen({ judge, seconds, prompt, blanks, hand, picked, others, onPick }: BadTakesPlayScreenProps) {
  const answers = picked.map((id) => hand.find((c) => c.id === id)?.text);
  return (
    <Screen world={WORLD}
      glance={<><TopBar timer={<Countdown seconds={seconds} />}><JudgeBadge judge={judge} /></TopBar><PromptCard text={prompt} answers={answers} numbered={blanks === 2} /></>}
      thumb={<>{blanks === 2 ? <Strip>Pick two cards, in order</Strip> : <PlayedStrip players={others} />}<AnswerHand cards={hand} picked={picked} numbered={blanks === 2} onPick={onPick} /></>} />
  );
}

export interface BadTakesWaitScreenProps {
  judge: PlayerView;
  isJudge: boolean;
  seconds: number;
  prompt: string;
  others: (PlayerView & { played: boolean })[];
  /** Your answers, for people who have played. */
  mine?: string[];
  /** Everyone has played. The judge sees the pull button. */
  allIn?: boolean;
  onPull?: () => void;
}
/** While people play. Pass a ref and call dropIn each time someone plays. */
export const BadTakesWaitScreen = forwardRef<CardBagHandle, BadTakesWaitScreenProps>(function BadTakesWaitScreen({ judge, isJudge, seconds, prompt, others, mine, allIn, onPull }, ref) {
  return (
    <Screen world={WORLD}
      glance={<><TopBar timer={allIn ? undefined : <Countdown seconds={seconds} />}><JudgeBadge judge={judge} isMe={isJudge} /></TopBar><PromptCard text={prompt} /></>}
      thumb={<><PlayedStrip players={others} />{mine && !isJudge ? <Strip>You played: {mine.join(' / ')}</Strip> : null}<CardBag ref={ref} />
        {isJudge ? (allIn ? <Button onClick={onPull}>Pull the first card</Button> : <Strip center>Wait for the answers</Strip>) : (allIn ? <Strip center>{judge.nickname} pulls the cards out next</Strip> : null)}</>} />
  );
});

export interface BadTakesRevealScreenProps {
  judge: PlayerView;
  isJudge: boolean;
  prompt: string;
  /** Answers shown so far in the blank. Empty while the card is on its way. */
  answers: string[];
  shown: number;
  total: number;
  numbered?: boolean;
  onNext?: () => void;
}
/** Cards come out one at a time. Pass a ref and call pullOut with the card's text. */
export const BadTakesRevealScreen = forwardRef<CardBagHandle, BadTakesRevealScreenProps>(function BadTakesRevealScreen({ judge, isJudge, prompt, answers, shown, total, numbered, onNext }, ref) {
  const last = shown >= total;
  return (
    <Screen world={WORLD}
      glance={<><TopBar><JudgeBadge judge={judge} isMe={isJudge} /></TopBar><PromptCard text={prompt} answers={answers} numbered={numbered} /></>}
      thumb={<><RevealProgress total={total} shown={shown} /><CardBag ref={ref} />
        {isJudge ? <Button onClick={onNext}>{last ? `Show all ${total} cards` : 'Pull the next card'}</Button> : <Strip center>{judge.nickname} is reading it out</Strip>}</>} />
  );
});

export interface PlayedAnswer { id: string; texts: string[]; }
export interface BadTakesJudgingScreenProps {
  judge: PlayerView;
  isJudge: boolean;
  seconds: number;
  prompt: string;
  answers: PlayedAnswer[];
  /** The judge's current choice. Other players never see it. */
  selectedId?: string;
  onSelect?: (id: string) => void;
  onConfirm?: () => void;
}
/** All cards, no names. The judge taps one, then gives it the point. */
export function BadTakesJudgingScreen({ judge, isJudge, seconds, prompt, answers, selectedId, onSelect, onConfirm }: BadTakesJudgingScreenProps) {
  const chosen = isJudge ? answers.find((a) => a.id === selectedId) : undefined;
  return (
    <Screen world={WORLD}
      glance={<><TopBar timer={<Countdown seconds={seconds} />}><JudgeBadge judge={judge} isMe={isJudge} /></TopBar><PromptCard text={prompt} answers={chosen?.texts} /></>}
      thumb={<><div className={styles.choose}>{answers.map((a) => <AnswerCard key={a.id} text={a.texts.join(' / ')} selected={isJudge && a.id === selectedId} onSelect={isJudge ? () => onSelect?.(a.id) : undefined} />)}</div>
        {isJudge ? <Button disabled={!selectedId} onClick={onConfirm}>{selectedId ? 'Give it the point' : 'Tap the best card'}</Button> : <Strip center>{judge.nickname} is choosing</Strip>}</>} />
  );
}

export interface BadTakesWinnerScreenProps {
  winner: PlayerView;
  prompt: string;
  answers: string[];
  rows: ScoreRow[];
  /** "Time's up! Random pick" when the judge ran out of time. */
  note?: string;
}
/** Four seconds: the winning sentence, who played it, and the score going up. */
export function BadTakesWinnerScreen({ winner, prompt, answers, rows, note }: BadTakesWinnerScreenProps) {
  return (
    <Screen world={WORLD}
      glance={<><WinnerBanner player={winner} note={note} /><PromptCard text={prompt} answers={answers} size="big" /></>}
      thumb={<Scoreboard rows={rows} />} />
  );
}
