// Who won the round: their answer in the prompt, and the scores with their +1
import { BadTakesWinnerScreen, type PlayerView, type ScoreRow } from '../../ui'
import type { BadTakesEvent, BadTakesView } from './protocol'

interface WinnerProps {
  view: BadTakesView
  winner: PlayerView
  rows: ScoreRow[]
  // The latest game event, a new object each time one arrives
  event: { number: number; event: unknown } | null
}

export function Winner({ view, winner, rows, event }: WinnerProps) {
  if (!view.prompt) return null
  // Only the reveal event says whether the judge ran out of time
  const revealed = event?.event as BadTakesEvent | undefined
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
