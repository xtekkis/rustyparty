// While people play: their cards drop into the bag. Once everyone's in, the judge pulls the first card
import { useEffect, useRef } from 'react'
import { BadTakesWaitScreen, type CardBagHandle, type PlayerView } from '../../ui'
import type { BadTakesAction, BadTakesEvent, BadTakesView } from './protocol'

interface WaitProps {
  view: BadTakesView
  judge: PlayerView
  isJudge: boolean
  others: (PlayerView & { played: boolean })[]
  seconds: number
  // The latest game event, a new object each time one arrives
  event: { number: number; event: unknown } | null
  onAction: (action: BadTakesAction) => void
}

export function Wait({ view, judge, isJudge, others, seconds, event, onAction }: WaitProps) {
  const bag = useRef<CardBagHandle>(null)

  // Each time someone plays, a face-down card flies from their face on the strip into the bag
  useEffect(() => {
    const played = event?.event as BadTakesEvent | undefined
    if (played?.event !== 'submitted') return
    void bag.current?.dropIn(document.querySelector(`[data-player="${played.player}"]`))
  }, [event])

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
