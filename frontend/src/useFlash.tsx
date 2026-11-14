// A short message at the top of the screen that goes away by itself
import { useCallback, useEffect, useState } from 'react'
import { StatusBanner } from './ui'

interface Flash {
  text: string
  tone: 'info' | 'alert'
}

// Errors stay a little longer than good news
const SHOWN_FOR_MS = { info: 2500, alert: 4000 }

export function useFlash() {
  const [flash, setFlash] = useState<Flash | null>(null)

  useEffect(() => {
    if (!flash) return
    const timer = setTimeout(() => setFlash(null), SHOWN_FOR_MS[flash.tone])
    return () => clearTimeout(timer)
  }, [flash])

  const banner = flash ? (
    // Outside any screen, so it brings the platform's colours and fonts with it
    <div data-theme="platform">
      <StatusBanner tone={flash.tone}>{flash.text}</StatusBanner>
    </div>
  ) : null

  // The same function every render, so effects can depend on it
  const show = useCallback(
    (text: string, tone: Flash['tone'] = 'info') => setFlash({ text, tone }),
    [],
  )

  return { banner, show }
}
