// The server's HTTP endpoints. Everything else goes over the WebSocket
import type { CreateRoomResponse, HttpError } from './protocol'

const UNREACHABLE = "Can't reach the server, check your connection"

// A new room with a fresh code. Throws an error whose message can be shown as it is
export async function createRoom(): Promise<string> {
  let response: Response
  try {
    response = await fetch('/rooms', { method: 'POST' })
  } catch {
    throw new Error(UNREACHABLE)
  }
  if (!response.ok) {
    // The server's errors are JSON. Anything else came from something in between, like a proxy
    const error = (await response.json().catch(() => null)) as HttpError | null
    throw new Error(error?.message ?? UNREACHABLE)
  }
  const room = (await response.json()) as CreateRoomResponse
  return room.code
}
