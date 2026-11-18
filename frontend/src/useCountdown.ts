// Seconds left on a countdown the server sent, ticking down until the next message corrects it
import { useEffect, useState } from 'react'

// Often enough that each second changes on time
const TICK_MS = 250

// seconds: what the server said; at: when that message arrived, in ms
export function useCountdown(seconds: number | null | undefined, at: number): number {
  const [now, setNow] = useState(Date.now)
  const running = seconds !== null && seconds !== undefined && seconds > 0

  useEffect(() => {
    if (!running) return
    const timer = setInterval(() => setNow(Date.now()), TICK_MS)
    return () => clearInterval(timer)
  }, [running, at])

  if (!running) return 0
  const gone = Math.max(0, (now - at) / 1000)
  return Math.max(0, Math.ceil(seconds - gone))
}
