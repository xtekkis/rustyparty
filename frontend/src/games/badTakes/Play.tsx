// Playing your cards: tap to pick one, or two in order for two blanks, then play them with the button
import { useState } from 'react'
import { BadTakesPlayScreen, type PlayerView } from '../../ui'
import type { BadTakesAction, BadTakesView } from './protocol'

interface PlayProps {
  view: BadTakesView
  judge: PlayerView
  // Everyone playing this round but the judge, and whether they've played yet
  others: (PlayerView & { played: boolean })[]
  seconds: number
  onAction: (action: BadTakesAction) => void
}

export function Play({ view, judge, others, seconds, onAction }: PlayProps) {
  // Cards picked so far, in order
  const [picked, setPicked] = useState<string[]>([])
  // The cards have gone to the server, so the button can't send them again
  const [sent, setSent] = useState(false)
  if (!view.prompt || !view.hand) return null
  const blanks = view.prompt.pick

  function pick(id: string) {
    // Tapping a picked card takes it back
    if (picked.includes(id)) {
      setPicked(picked.filter((card) => card !== id))
      return
    }
    // Fill the blanks in order. Once they're full, a new card takes the last one's place
    setPicked(picked.length < blanks ? [...picked, id] : [...picked.slice(0, blanks - 1), id])
  }

  function play() {
    if (picked.length !== blanks || sent) return
    setSent(true)
    onAction({ kind: 'submit', cards: picked.map(Number) })
  }

  return (
    <BadTakesPlayScreen
      judge={judge}
      seconds={seconds}
      prompt={view.prompt.text}
      blanks={blanks}
      hand={view.hand.map((card) => ({ id: String(card.id), text: card.text }))}
      picked={picked}
      others={others}
      onPick={pick}
      onPlay={play}
      sent={sent}
    />
  )
}
