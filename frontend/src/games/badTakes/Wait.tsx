// While people play: their cards drop into the bag. Once everyone's in, the judge pulls the first card
import { useRef } from 'react'
import type { GameEvent } from '../../serverState'
import { BadTakesWaitScreen, type CardBagHandle, type PlayerView } from '../../ui'
import { useNewEvents } from '../../useNewEvents'
import type { BadTakesAction, BadTakesEvent, BadTakesView } from './protocol'

interface WaitProps {
  view: BadTakesView
  judge: PlayerView
  isJudge: boolean
  others: (PlayerView & { played: boolean })[]
  seconds: number
  events: GameEvent[]
  onAction: (action: BadTakesAction) => void
}

export function Wait({ view, judge, isJudge, others, seconds, events, onAction }: WaitProps) {
  const bag = useRef<CardBagHandle>(null)

  // Each time someone plays, a face-down card flies from their face on the strip into the bag
  useNewEvents(events, (event) => {
    const played = event as BadTakesEvent
    if (played.event !== 'submitted') return
    void bag.current?.dropIn(document.querySelector(`[data-player="${played.player}"]`))
  })

  if (!view.prompt) return null
  return (
    <BadTakesWaitScreen
      ref={bag}
      judge={judge}
      isJudge={isJudge}
      seconds={seconds}
      prompt={view.prompt.text}
      others={others}
      mine={view.my_submission?.map((card) => card.text)}
      // The server starts reading once everyone has played or time is up
      allIn={view.phase === 'reading'}
      onPull={() => onAction({ kind: 'pull' })}
    />
  )
}
