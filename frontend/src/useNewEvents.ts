// Runs a handler once for each game event that arrived since the screen appeared, in order
import { useEffect, useRef } from 'react'
import type { GameEvent } from './serverState'

export function useNewEvents(events: GameEvent[], onEvent: (event: unknown) => void) {
  // Events from before the screen appeared aren't replayed
  const seen = useRef(events.at(-1)?.number ?? 0)
  // The latest handler, so a new one each render doesn't matter
  const handler = useRef(onEvent)
  useEffect(() => {
    handler.current = onEvent
  })

  useEffect(() => {
    for (const { number, event } of events) {
      if (number <= seen.current) continue
      seen.current = number
      handler.current(event)
    }
  }, [events])
}
