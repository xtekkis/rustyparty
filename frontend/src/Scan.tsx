// Scanning the host's QR code with the camera. The QR reader is only downloaded once someone taps Scan
import { useEffect, useRef, useState } from 'react'
import { codeFromText } from './joinLink'
import { ScanScreen } from './ui'

// How often a camera frame is checked for a QR code
const SCAN_EVERY_MS = 250
// Frames are shrunk to this width first: plenty for a QR code, and much faster to read
const FRAME_WIDTH = 480

interface ScanProps {
  onFound: (code: string) => void
  onTypeCode: () => void
  onBack: () => void
}

export function Scan({ onFound, onTypeCode, onBack }: ScanProps) {
  const video = useRef<HTMLVideoElement>(null)
  const [blocked, setBlocked] = useState(false)
  // The latest onFound, so a new one from the parent doesn't restart the camera
  const found = useRef(onFound)
  useEffect(() => {
    found.current = onFound
  })

  useEffect(() => {
    let stream: MediaStream | null = null
    let timer: ReturnType<typeof setInterval> | undefined
    let stopped = false

    async function start() {
      try {
        // The back camera. Browsers only allow it on https or localhost
        stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'environment' } })
      } catch {
        if (!stopped) setBlocked(true)
        return
      }
      const { default: jsQR } = await import('jsqr')
      const element = video.current
      if (stopped || !element) {
        stream.getTracks().forEach((track) => track.stop())
        return
      }
      element.srcObject = stream
      await element.play().catch(() => {})

      const canvas = document.createElement('canvas')
      const context = canvas.getContext('2d', { willReadFrequently: true })
      timer = setInterval(() => {
        if (!context || element.videoWidth === 0) return
        canvas.width = FRAME_WIDTH
        canvas.height = Math.round((FRAME_WIDTH * element.videoHeight) / element.videoWidth)
        context.drawImage(element, 0, 0, canvas.width, canvas.height)
        const frame = context.getImageData(0, 0, canvas.width, canvas.height)
        const qr = jsQR(frame.data, frame.width, frame.height)
        // Any other QR code is ignored, and scanning goes on
        const code = qr ? codeFromText(qr.data) : null
        if (code) {
          clearInterval(timer)
          found.current(code)
        }
      }, SCAN_EVERY_MS)
    }

    void start()
    return () => {
      stopped = true
      clearInterval(timer)
      stream?.getTracks().forEach((track) => track.stop())
    }
  }, [])

  return (
    <ScanScreen
      camera={blocked ? undefined : <video ref={video} muted playsInline />}
      blocked={blocked}
      onTypeCode={onTypeCode}
      onBack={onBack}
    />
  )
}
