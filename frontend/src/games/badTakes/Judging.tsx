// All the answers together, with no names. The judge picks one, then gives it the point
import { useState } from 'react'
import { BadTakesJudgingScreen, type PlayerView } from '../../ui'
import type { BadTakesAction, BadTakesView } from './protocol'

interface JudgingProps {
  view: BadTakesView
  judge: PlayerView
  isJudge: boolean
  seconds: number
  onAction: (action: BadTakesAction) => void
  // Opens the invite card
  onInvite: () => void
}

export function Judging({ view, judge, isJudge, seconds, onAction, onInvite }: JudgingProps) {
  // The judge's choice so far. It stays on this phone until they give it the point
  const [selected, setSelected] = useState<string>()
  if (!view.prompt) return null

  // Any card of an answer stands for the whole answer
  const answers = view.cards_to_judge.map((cards) => ({
    id: String(cards[0].id),
    texts: cards.map((card) => card.text),
  }))

  return (
    <BadTakesJudgingScreen
      onInvite={onInvite}
      judge={judge}
      isJudge={isJudge}
      seconds={seconds}
      prompt={view.prompt.text}
      answers={answers}
      selectedId={selected}
      onSelect={setSelected}
      onConfirm={() => {
        if (selected) onAction({ kind: 'pick', card: Number(selected) })
      }}
    />
  )
}
