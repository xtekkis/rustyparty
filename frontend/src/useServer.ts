// The one connection to the server. Screens read its state and send through it
import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import type { ClientMessage, ServerMessage } from './protocol'
import { applyMessage, initialState } from './serverState'

export type ConnectionStatus = 'connecting' | 'open' | 'closed'

// What screens get from the hook
export type Server = ReturnType<typeof useServer>

// Same address as the page. In development Vite passes /ws on to the backend
function serverUrl(): string {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${location.host}/ws`
}

interface Received {
  message: ServerMessage
  at: number
}

export function useServer() {
  const [state, dispatch] = useReducer(
    (current: typeof initialState, received: Received) =>
      applyMessage(current, received.message, received.at),
    initialState,
  )
  const [status, setStatus] = useState<ConnectionStatus>('connecting')
  const socket = useRef<WebSocket | null>(null)

  useEffect(() => {
    const ws = new WebSocket(serverUrl())
    socket.current = ws
    ws.onopen = () => setStatus('open')
    ws.onclose = () => setStatus('closed')
    ws.onmessage = (message) =>
      dispatch({ message: JSON.parse(message.data) as ServerMessage, at: Date.now() })
    return () => {
      // A socket closed here is replaced straight away, so it mustn't report itself closed
      ws.onopen = null
      ws.onclose = null
      ws.onmessage = null
      ws.close()
    }
  }, [])

  // Dropped while the connection isn't open
  const send = useCallback((message: ClientMessage) => {
    if (socket.current?.readyState === WebSocket.OPEN) {
      socket.current.send(JSON.stringify(message))
    }
  }, [])

  return { status, state, send }
}
