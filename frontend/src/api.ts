// The server's HTTP endpoints. Everything else goes over the WebSocket
import type { CreateRoomResponse, HttpError } from './protocol'

const UNREACHABLE = "Can't reach the server, check your connection"

// Every call throws an error whose message can be shown as it is
async function request(url: string, options?: RequestInit): Promise<Response> {
  let response: Response
  try {
    response = await fetch(url, options)
  } catch {
    throw new Error(UNREACHABLE)
  }
  if (!response.ok) {
    // The server's errors are JSON. Anything else came from something in between, like a proxy
    const error = (await response.json().catch(() => null)) as HttpError | null
    throw new Error(error?.message ?? UNREACHABLE)
  }
  return response
}

// A new room with a fresh code
export async function createRoom(): Promise<string> {
  const response = await request('/rooms', { method: 'POST' })
  const room = (await response.json()) as CreateRoomResponse
  return room.code
}

// Throws "No room with that code" if the room doesn't exist
export async function checkRoom(code: string): Promise<void> {
  await request(`/rooms/${encodeURIComponent(code)}`)
}
