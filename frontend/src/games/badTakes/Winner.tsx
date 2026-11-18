// Who won the round: their answer in the prompt, and the scores with their +1
import type { GameEvent } from '../../serverState'
import { BadTakesWinnerScreen, type PlayerView, type ScoreRow } from '../../ui'
import type { BadTakesEvent, BadTakesView } from './protocol'

interface WinnerProps {
  view: BadTakesView
  winner: PlayerView
  rows: ScoreRow[]
  events: GameEvent[]
}

export function Winner({ view, winner, rows, events }: WinnerProps) {
  if (!view.prompt) return null
  // Only the reveal event says whether the judge ran out of time
  const revealed = events
    .map((candidate) => candidate.event as BadTakesEvent)
    .findLast((candidate) => candidate.event === 'winner_revealed')
  const timedOut = revealed?.event === 'winner_revealed' && revealed.judge_timed_out

  return (
    <BadTakesWinnerScreen
      winner={winner}
      prompt={view.prompt.text}
      answers={(view.winning_cards ?? []).map((card) => card.text)}
      rows={rows.map((row) => ({ ...row, gained: row.player.id === winner.id }))}
      note={timedOut ? "Time's up! Random pick" : undefined}
    />
  )
}
