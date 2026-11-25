// The one connection to the server. Screens read its state and send through it.
// It also holds this phone's seat, and gets it back whenever the connection drops
import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import type { ClientMessage, ServerMessage } from './protocol'
import { applyMessage, initialState } from './serverState'
import { clearSession, loadSession, saveSession, type Session } from './session'

export type ConnectionStatus = 'connecting' | 'open' | 'closed'

// What screens get from the hook
export type Server = ReturnType<typeof useServer>

// Seconds to wait before each try to reconnect. The last one repeats
const RETRY_SECONDS = [1, 2, 4, 8]
// Away from the app this long, an open-looking connection may be dead, so a fresh one is made
const STALE_AFTER_MS = 5000

// Same address as the page. In development Vite passes /ws on to the backend
function serverUrl(): string {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${location.host}/ws`
}

// What to tell a phone whose seat is gone when it tries to get it back
function lostMessage(error: { code: string; message: string }): string {
  if (error.code === 'room_not_found') return 'That room has closed'
  if (error.code === 'banned_from_room') return error.message
  return 'You were away too long. Join the room again'
}

interface Received {
  message: ServerMessage
  at: number
}

// openedFor is the room of the invite link the page was opened with.
// A saved seat in another room isn't used then, the invite wins
export function useServer(openedFor: string | null, onSeatLost: (message: string) => void) {
  const [state, dispatch] = useReducer(
    (current: typeof initialState, received: Received | 'forget') =>
      received === 'forget' ? initialState : applyMessage(current, received.message, received.at),
    initialState,
  )
  const [status, setStatus] = useState<ConnectionStatus>('connecting')
  const socket = useRef<WebSocket | null>(null)
  // Counts up for each new connection
  const [connection, setConnection] = useState(0)
  const retries = useRef(0)
  // Set once the server has closed this connection on purpose
  const stopped = useRef(false)

  // The room this phone is in and the token that gets it back in.
  // The ref is for the socket's handlers, the state is for screens
  const [seat, setSeat] = useState<Session | null>(() => {
    const saved = loadSession()
    return saved && (!openedFor || saved.room === openedFor) ? saved : null
  })
  const seatNow = useRef(seat)
  const changeSeat = useCallback((next: Session | null) => {
    seatNow.current = next
    setSeat(next)
  }, [])
  // The room of a join that hasn't been answered yet
  const joining = useRef<string | null>(null)
  // True while the join is this phone asking for its seat back
  const rejoining = useRef(false)

  // The latest onSeatLost, so a new one from the parent doesn't reconnect
  const seatLost = useRef(onSeatLost)
  useEffect(() => {
    seatLost.current = onSeatLost
  })

  useEffect(() => {
    const ws = new WebSocket(serverUrl())
    socket.current = ws
    stopped.current = false
    let retry: ReturnType<typeof setTimeout> | undefined

    ws.onopen = () => {
      setStatus('open')
      const seat = seatNow.current
      // Without a seat, being connected is all there is to it. With one, the waits between
      // tries only start over once the seat is back, so a refused rejoin waits longer each time
      if (!seat) {
        retries.current = 0
        return
      }
      // With the token the server ignores the name and avatar
      joining.current = seat.room
      rejoining.current = true
      const join: ClientMessage = { type: 'join', room: seat.room, nickname: '', avatar: null, token: seat.token }
      ws.send(JSON.stringify(join))
    }

    ws.onclose = () => {
      setStatus('closed')
      if (stopped.current) return
      const wait = RETRY_SECONDS[Math.min(retries.current, RETRY_SECONDS.length - 1)]
      retries.current += 1
      retry = setTimeout(() => {
        setStatus('connecting')
        setConnection((count) => count + 1)
      }, wait * 1000)
    }

    ws.onmessage = (event) => {
      let message = JSON.parse(event.data) as ServerMessage
      // A join dropped for coming too fast gets no answer of its own, so this stands in for it
      if (message.type === 'slow_down' && joining.current) {
        message = { type: 'error', code: 'slow_down', message: 'Too many tries, wait a moment and try again' }
      }
      if (message.type === 'welcome' && joining.current) {
        const next = { room: joining.current, token: message.token }
        saveSession(next)
        changeSeat(next)
        joining.current = null
        rejoining.current = false
        retries.current = 0
      } else if (message.type === 'error' && joining.current) {
        joining.current = null
        if (rejoining.current && message.code === 'slow_down') {
          // The server is busy with this address, not saying the seat is gone: keep it and
          // try again, the usual way after a drop
          rejoining.current = false
          ws.close()
          return
        }
        if (rejoining.current) {
          // The seat is gone. Start over at home, without the old room's state
          rejoining.current = false
          // A ban stays saved, so joining that room again is still refused
          if (message.code !== 'banned_from_room') clearSession()
          changeSeat(null)
          retries.current = 0
          dispatch('forget')
          seatLost.current(lostMessage(message))
          return
        }
      } else if (message.type === 'kicked' || message.type === 'banned' || message.type === 'replaced') {
        // The server closes the connection after these, and coming back would undo them
        stopped.current = true
        if (message.type === 'kicked') clearSession()
        if (message.type !== 'replaced') changeSeat(null)
      }
      dispatch({ message, at: Date.now() })
    }

    return () => {
      // A socket closed here is replaced straight away, so it mustn't report itself closed
      ws.onopen = null
      ws.onclose = null
      ws.onmessage = null
      ws.close()
      clearTimeout(retry)
    }
  }, [connection, changeSeat])

  // Back in the app after the phone was locked or another app was open
  useEffect(() => {
    let hiddenAt = 0
    function changed() {
      if (document.hidden) {
        hiddenAt = Date.now()
        return
      }
      if (stopped.current) return
      const state = socket.current?.readyState
      const working = state === WebSocket.OPEN || state === WebSocket.CONNECTING
      if (working && Date.now() - hiddenAt < STALE_AFTER_MS) return
      // Straight away, without waiting for the next try
      retries.current = 0
      setStatus('connecting')
      setConnection((count) => count + 1)
    }
    document.addEventListener('visibilitychange', changed)
    return () => document.removeEventListener('visibilitychange', changed)
  }, [])

  // Dropped while the connection isn't open
  const send = useCallback((message: ClientMessage) => {
    if (socket.current?.readyState !== WebSocket.OPEN) return
    if (message.type === 'join') joining.current = message.room
    socket.current.send(JSON.stringify(message))
  }, [])

  // Start again with nothing from the old room, like after being removed from it
  const restart = useCallback(() => {
    if (seatNow.current) {
      // Only a replaced seat is still held after the server stops, and it stays saved:
      // another tab in this browser may be the one using it
      if (!stopped.current) clearSession()
      changeSeat(null)
    }
    joining.current = null
    rejoining.current = false
    dispatch('forget')
    setStatus('connecting')
    setConnection((count) => count + 1)
  }, [changeSeat])

  // Take the seat back from the tab or phone that opened it since
  const takeBack = useCallback(() => {
    dispatch('forget')
    retries.current = 0
    setStatus('connecting')
    setConnection((count) => count + 1)
  }, [])

  return { status, state, send, restart, takeBack, room: seat?.room ?? null }
}