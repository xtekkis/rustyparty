// Keeps the phone's screen on while active, where the browser allows it. Without it the game
// still works, the screen just dims on its usual timer
import { useEffect } from 'react'

export function useWakeLock(active: boolean) {
  useEffect(() => {
    if (!active || !('wakeLock' in navigator)) return
    let lock: WakeLockSentinel | null = null
    let stopped = false

    async function take() {
      try {
        const taken = await navigator.wakeLock.request('screen')
        if (stopped) void taken.release()
        else lock = taken
      } catch {
        // Not allowed here, like on plain http or in battery saver
      }
    }

    // The browser drops the lock whenever the page is hidden, so take it again when it's back
    function visible() {
      if (document.visibilityState === 'visible') void take()
    }

    void take()
    document.addEventListener('visibilitychange', visible)
    return () => {
      stopped = true
      document.removeEventListener('visibilitychange', visible)
      void lock?.release()
    }
  }, [active])
}
