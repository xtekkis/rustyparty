// True once on has stayed true for a while, and false again as soon as it isn't
import { useEffect, useState } from 'react'

export function useLasting(on: boolean, ms: number): boolean {
  const [lasted, setLasted] = useState(false)
  useEffect(() => {
    if (!on) return
    const timer = setTimeout(() => setLasted(true), ms)
    return () => {
      clearTimeout(timer)
      setLasted(false)
    }
  }, [on, ms])
  return on && lasted
}