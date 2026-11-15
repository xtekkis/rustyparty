// True for a short beat after the last vote ended, while its result is on screen
import { useEffect, useState } from 'react'
import type { Server } from './useServer'

// "Don't linger. This is a beat, not a stop."
const BEAT_MS = 2500

export function useVoteBeat(server: Server): boolean {
  const at = server.state.voteEnd?.at
  // The end time whose beat is over
  const [over, setOver] = useState<number | undefined>()
  useEffect(() => {
    if (at === undefined) return
    const timer = setTimeout(() => setOver(at), Math.max(0, at + BEAT_MS - Date.now()))
    return () => clearTimeout(timer)
  }, [at])
  return at !== undefined && over !== at
}
