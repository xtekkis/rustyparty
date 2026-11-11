// Typing or pasting a room code. A full code is checked with the server straight away
import { useEffect, useState } from 'react'
import { checkRoom } from './api'
import { JoinCodeScreen } from './ui'

const CODE_LENGTH = 4

// A bare code like "kxtp", or an invite link ending in /join/KXTP
function codeFromText(text: string): string | null {
  const match = text.trim().match(/(?:^|\/join\/)([a-z]{4})$/i)
  return match ? match[1].toUpperCase() : null
}

interface JoinRoomProps {
  onFound: (code: string) => void
  onBack: () => void
}

export function JoinRoom({ onFound, onBack }: JoinRoomProps) {
  const [code, setCode] = useState('')
  const [error, setError] = useState<string | undefined>()
  const [checking, setChecking] = useState(false)

  async function check(full: string) {
    setCode(full)
    setChecking(true)
    try {
      await checkRoom(full)
      onFound(full)
    } catch (failure) {
      setError((failure as Error).message)
    } finally {
      setChecking(false)
    }
  }

  function letter(next: string) {
    if (checking) return
    // After an error, the next letter starts a fresh code
    const typed = error ? next : code + next
    setError(undefined)
    if (typed.length === CODE_LENGTH) {
      void check(typed)
    } else if (typed.length < CODE_LENGTH) {
      setCode(typed)
    }
  }

  function remove() {
    if (checking) return
    setError(undefined)
    setCode(error ? '' : code.slice(0, -1))
  }

  useEffect(() => {
    function paste(event: ClipboardEvent) {
      const pasted = codeFromText(event.clipboardData?.getData('text') ?? '')
      if (pasted && !checking) {
        setError(undefined)
        void check(pasted)
      }
    }
    document.addEventListener('paste', paste)
    return () => document.removeEventListener('paste', paste)
  })

  return (
    <JoinCodeScreen
      code={code}
      error={error}
      checking={checking}
      onLetter={letter}
      onDelete={remove}
      // The camera screen comes in a later step
      onScan={() => {}}
      onBack={onBack}
    />
  )
}
