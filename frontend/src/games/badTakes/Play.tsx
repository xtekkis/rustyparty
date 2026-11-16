// Playing your cards: one tap for a one-blank prompt, two taps in order for two blanks
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
  // Cards picked so far for a two-blank prompt, in order
  const [picked, setPicked] = useState<string[]>([])
  if (!view.prompt || !view.hand) return null
  const blanks = view.prompt.pick

  function pick(id: string) {
    // Tapping a picked card takes it back
    if (picked.includes(id)) {
      setPicked(picked.filter((card) => card !== id))
      return
    }
    const cards = [...picked, id]
    if (cards.length === blanks) {
      onAction({ kind: 'submit', cards: cards.map(Number) })
    } else {
      setPicked(cards)
    }
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
    />
  )
}
