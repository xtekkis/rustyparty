// The judge pulls the answers out of the bag one at a time, and each lands in the prompt
import { useEffect, useRef, useState } from 'react'
import type { GameEvent } from '../../serverState'
import { BadTakesRevealScreen, type CardBagHandle, type PlayerView } from '../../ui'
import type { BadTakesAction, BadTakesEvent, BadTakesView } from './protocol'

interface RevealProps {
  view: BadTakesView
  judge: PlayerView
  isJudge: boolean
  events: GameEvent[]
  onAction: (action: BadTakesAction) => void
  // Opens the invite card
  onInvite: () => void
}

type Pulled = Extract<BadTakesEvent, { event: 'answer_pulled' }>
const pulledIn = (event: GameEvent | undefined) => {
  const game = event?.event as BadTakesEvent | undefined
  return game?.event === 'answer_pulled' ? game : null
}

export function Reveal({ view, judge, isJudge, events, onAction, onInvite }: RevealProps) {
  // The latest pull, the same object until the next one arrives
  const event = events.findLast((candidate) => pulledIn(candidate))
  const bag = useRef<CardBagHandle>(null)
  // The number of the pull event whose card has landed in the prompt
  const [landed, setLanded] = useState<number | null>(null)

  // Each pull flies its card out of the bag, and it fills the blank once it lands
  useEffect(() => {
    const pull = pulledIn(event)
    if (!event || !pull) return
    // An animation from an older effect (or React's development double run) never counts as landing
    let current = true
    // A card still flying lands at once, so a fast tap never gets stuck
    bag.current?.finish()
    void bag.current
      ?.pullOut(pull.cards.map((card) => card.text).join(' / '), document.querySelector('[data-prompt]'))
      .then(() => {
        if (current) setLanded(event.number)
      })
    return () => {
      current = false
    }
  }, [event])

  if (!view.prompt) return null
  const pulled = view.cards_to_judge.length
  const total = pulled + view.in_bag

  // While a card is in the air the blank keeps the answer before it. The event comes before the
  // server's new view, so the card is looked for in the list rather than assumed to be last
  const pull: Pulled | null = pulledIn(event)
  let out = view.cards_to_judge
  if (pull && event && landed !== event.number) {
    const at = out.findIndex((cards) => cards[0]?.id === pull.cards[0]?.id)
    if (at >= 0) out = out.slice(0, at)
  }
  const showing = out.length > 0 ? out[out.length - 1] : []
  function next() {
    bag.current?.finish()
    onAction(view.in_bag > 0 ? { kind: 'pull' } : { kind: 'show_all' })
  }

  return (
    <BadTakesRevealScreen
      onInvite={onInvite}
      ref={bag}
      judge={judge}
      isJudge={isJudge}
      prompt={view.prompt.text}
      answers={showing.map((card) => card.text)}
      shown={pulled}
      total={total}
      numbered={view.prompt.pick === 2}
      onNext={next}
    />
  )
}
