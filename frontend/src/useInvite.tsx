// The room's invite link: copy it, share it, or show it big with its QR code
import { QRCodeSVG } from 'qrcode.react'
import { useState } from 'react'
import { copyText } from './clipboard'
import { joinLink } from './joinLink'
import { InviteCard, type World } from './ui'

type Show = (text: string, tone?: 'info' | 'alert') => void

// world: the screens the invite card shows over. show: the screen's flash messages
export function useInvite(code: string, world: World, show: Show) {
  const [open, setOpen] = useState(false)
  const link = joinLink(code)

  async function copy() {
    if (await copyText(link)) {
      show('Invite link copied')
    } else {
      show("Couldn't copy the link", 'alert')
    }
  }

  // The phone's own share menu, only on browsers that have one
  const canShare = typeof navigator.share === 'function'
  async function share() {
    try {
      await navigator.share({ title: 'rustyparty', text: `Join my rustyparty room ${code}`, url: link })
    } catch (error) {
      // Closing the menu without picking anything isn't a failure
      if ((error as Error).name !== 'AbortError') show("Couldn't share the link", 'alert')
    }
  }

  const qr = <QRCodeSVG value={link} title={`Join link ${link}`} />
  const onShare = canShare ? share : undefined
  const card = open ? (
    <InviteCard world={world} code={code} qr={qr} onCopy={copy} onShare={onShare} onClose={() => setOpen(false)} />
  ) : null

  return { qr, copy, share: onShare, card, open: () => setOpen(true) }
}