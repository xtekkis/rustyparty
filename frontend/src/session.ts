// This phone's seat, kept across reloads so it gets the same seat back in the same room.
// A phone is in one room at a time, so joining another room replaces it
const KEY = 'rustyparty.session'

export interface Session {
  room: string
  token: string
}

export function loadSession(): Session | null {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? 'null') as Session | null
  } catch {
    // Storage can be blocked, like in some private windows
    return null
  }
}

// The saved token for this room, if this phone was in it
export function savedToken(room: string): string | null {
  const session = loadSession()
  return session?.room === room ? session.token : null
}

export function saveSession(session: Session) {
  try {
    localStorage.setItem(KEY, JSON.stringify(session))
  } catch {
    // Without storage the phone still plays, it just can't get its seat back after a reload
  }
}

export function clearSession() {
  try {
    localStorage.removeItem(KEY)
  } catch {
    // Nothing was saved then
  }
}