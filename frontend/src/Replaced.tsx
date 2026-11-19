// This phone's seat was opened in another tab or on another phone, which now has it
import { Button, Screen, Strip } from './ui'
import type { Server } from './useServer'

interface ReplacedProps {
  server: Server
  onHome: () => void
}

export function Replaced({ server, onHome }: ReplacedProps) {
  return (
    <Screen
      centerGlance
      glance={<Strip center>You opened this room in another tab or on another phone</Strip>}
      thumb={
        <>
          <Button onClick={server.takeBack}>Play here</Button>
          <Button variant="secondary" onClick={onHome}>
            Back to home
          </Button>
        </>
      }
    />
  )
}