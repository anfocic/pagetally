import type { PerformanceMetrics } from './types'

export function startPerformanceTracking(
  report: (metrics: PerformanceMetrics) => void,
): () => void {
  let reported = false
  const observers: PerformanceObserver[] = []

  const metrics: Required<PerformanceMetrics> = {
    lcp: 0,
    fcp: 0,
    cls: 0,
    inp: 0,
    ttfb: 0,
  }

  const onHidden = () => {
    if (document.visibilityState === 'hidden') flush()
  }
  let timer: ReturnType<typeof setTimeout> | null = null
  const cleanup = () => {
    document.removeEventListener('visibilitychange', onHidden)
    if (timer != null) clearTimeout(timer)
  }

  const flush = () => {
    if (reported) return
    reported = true
    observers.forEach((o) => o.disconnect())
    cleanup()

    const clean: PerformanceMetrics = {}
    if (metrics.lcp > 0) clean.lcp = Math.round(metrics.lcp)
    if (metrics.fcp > 0) clean.fcp = Math.round(metrics.fcp)
    if (metrics.cls > 0) clean.cls = Math.round(metrics.cls * 10000) / 10000
    if (metrics.inp > 0) clean.inp = Math.round(metrics.inp)
    if (metrics.ttfb > 0) clean.ttfb = Math.round(metrics.ttfb)

    if (Object.keys(clean).length > 0) report(clean)
  }

  // LCP
  try {
    const obs = new PerformanceObserver((list) => {
      const entries = list.getEntries()
      if (entries.length > 0) {
        metrics.lcp = entries[entries.length - 1]!.startTime
      }
    })
    obs.observe({ type: 'largest-contentful-paint', buffered: true })
    observers.push(obs)
  } catch {}

  // FCP
  try {
    const obs = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (entry.name === 'first-contentful-paint') {
          metrics.fcp = entry.startTime
        }
      }
    })
    obs.observe({ type: 'paint', buffered: true })
    observers.push(obs)
  } catch {}

  // CLS
  let clsSession = 0
  let clsLastTime = 0
  try {
    const obs = new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as PerformanceEntry[]) {
        const shift = entry as unknown as {
          hadRecentInput?: boolean
          value?: number
        }
        if (shift.hadRecentInput) continue
        if (typeof shift.value !== 'number') continue

        if (clsLastTime === 0 || entry.startTime - clsLastTime < 1000) {
          clsSession += shift.value
        } else {
          metrics.cls = Math.max(metrics.cls, clsSession)
          clsSession = shift.value
        }
        clsLastTime = entry.startTime
      }
      metrics.cls = Math.max(metrics.cls, clsSession)
    })
    obs.observe({ type: 'layout-shift', buffered: true })
    observers.push(obs)
  } catch {}

  // INP (via first-input)
  try {
    const obs = new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as PerformanceEntry[]) {
        const fi = entry as unknown as {
          processingStart?: number
        }
        if (fi.processingStart !== undefined) {
          metrics.inp = fi.processingStart - entry.startTime
        }
      }
    })
    obs.observe({ type: 'first-input', buffered: true })
    observers.push(obs)
  } catch {}

  // TTFB — try sync read first, then observe in case nav entry isn't ready yet
  const readTtfb = (e?: PerformanceNavigationTiming) => {
    const nav =
      e ??
      (performance.getEntriesByType(
        'navigation',
      )[0] as PerformanceNavigationTiming | undefined)
    if (nav && nav.responseStart > 0 && metrics.ttfb === 0) {
      metrics.ttfb = nav.responseStart
    }
  }
  try {
    readTtfb()
    const obs = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        readTtfb(entry as PerformanceNavigationTiming)
      }
    })
    obs.observe({ type: 'navigation', buffered: true })
    observers.push(obs)
  } catch {}

  document.addEventListener('visibilitychange', onHidden)
  timer = setTimeout(flush, 15000)

  return () => {
    flush()
  }
}
