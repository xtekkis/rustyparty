// Invite links look like https://<site>/join/KXTP and open the profile screen for that room

export function joinLink(code: string): string {
  return `${location.origin}/join/${code}`
}

// The room code in this page's address, if it was opened from an invite link
export function codeFromAddress(): string | null {
  const match = location.pathname.match(/^\/join\/([a-z]{4})\/?$/i)
  return match ? match[1].toUpperCase() : null
}
