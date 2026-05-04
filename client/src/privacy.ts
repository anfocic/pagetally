export function stripQueryParams(path: string): string {
  const i = path.indexOf('?')
  return i === -1 ? path : path.slice(0, i)
}

export function getReferrerDomain(): string | undefined {
  try {
    if (!document.referrer) return undefined
    const url = new URL(document.referrer)
    if (url.hostname === location.hostname) return undefined
    return url.hostname
  } catch {
    return undefined
  }
}

export function getDeviceClass(w: number): 'mobile' | 'tablet' | 'desktop' {
  if (w < 640) return 'mobile'
  if (w < 1024) return 'tablet'
  return 'desktop'
}

export function roundViewportWidth(w: number): number {
  return Math.round(w / 10) * 10
}

export function checkDNT(): boolean {
  return (
    navigator.doNotTrack === '1' ||
    // @ts-expect-error legacy API
    navigator.globalPrivacyControl === true ||
    window.doNotTrack === '1'
  )
}
