import type { Payload } from './types'

export function sendPayload(payload: Payload, endpoint: string): void {
  const body = JSON.stringify(payload)
  const blob = new Blob([body], { type: 'application/json' })

  try {
    if (navigator.sendBeacon && navigator.sendBeacon(endpoint, blob)) return

    fetch(endpoint, {
      method: 'POST',
      body: blob,
      keepalive: true,
    }).catch(() => {})
  } catch {
    // silently ignore failures — analytics must never break the page
  }
}
