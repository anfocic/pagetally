const MAX_DUR_MS = 30 * 60 * 1000
const MIN_DUR_MS = 1_000

export interface Engagement {
  reset(path: string): void
  flush(): void
  stop(): void
}

export function startEngagement(
  send: (path: string, dur: number) => void,
): Engagement {
  let currentPath = ''
  let lastVisibleAt: number | null = null
  let accumulated = 0
  let flushed = true

  const now = () => Date.now()

  const accrue = () => {
    if (lastVisibleAt == null) return
    const delta = Math.min(now() - lastVisibleAt, MAX_DUR_MS)
    if (delta > 0) accumulated += delta
    lastVisibleAt = null
  }

  const flush = () => {
    accrue()
    if (flushed) return
    if (accumulated < MIN_DUR_MS) return
    const dur = Math.min(accumulated, MAX_DUR_MS)
    send(currentPath, dur)
    flushed = true
  }

  const onVisibility = () => {
    if (document.visibilityState === 'hidden') {
      flush()
    } else if (document.visibilityState === 'visible') {
      lastVisibleAt = now()
    }
  }

  const onPageHide = () => {
    flush()
  }

  const onPageShow = (e: PageTransitionEvent) => {
    if (e.persisted) {
      // bfcache restore — Analytics will fire a fresh pageview;
      // this just makes sure timer state is sane in case reset() lags.
      accumulated = 0
      flushed = false
      lastVisibleAt = document.visibilityState === 'visible' ? now() : null
    }
  }

  document.addEventListener('visibilitychange', onVisibility)
  window.addEventListener('pagehide', onPageHide)
  window.addEventListener('pageshow', onPageShow)

  return {
    reset(path: string) {
      currentPath = path
      accumulated = 0
      flushed = false
      lastVisibleAt = document.visibilityState === 'visible' ? now() : null
    },
    flush,
    stop() {
      document.removeEventListener('visibilitychange', onVisibility)
      window.removeEventListener('pagehide', onPageHide)
      window.removeEventListener('pageshow', onPageShow)
    },
  }
}
