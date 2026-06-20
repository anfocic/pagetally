import type { AnalyticsConfig, Payload, PerformanceMetrics } from './types'
import { checkDNT } from './privacy'
import { sendPayload } from './transport'
import {
  buildPageViewPayload,
  buildEventPayload,
  buildPerformancePayload,
  buildPageLeavePayload,
  getPath,
} from './payload'
import { startAutoTracking } from './collect'
import { startPerformanceTracking } from './performance'
import { startEngagement, type Engagement } from './engagement'
import { startScrollTracking, type ScrollTracker } from './scroll'
import { startClickTracking } from './clicks'

export type { AnalyticsConfig, Payload, PerformanceMetrics } from './types'

const INSTANCE_KEY = '__pagetally_active__'

export class Analytics {
  private config: Required<AnalyticsConfig>
  private cleanups: (() => void)[] = []
  private stopped = false
  private engagement: Engagement | null = null
  private scroll: ScrollTracker | null = null
  private lastViewPath = ''
  private lastViewTime = 0

  constructor(config: AnalyticsConfig) {
    if (!config.endpoint) {
      throw new Error('Analytics: endpoint is required')
    }
    if (!config.siteId) {
      throw new Error('Analytics: siteId is required')
    }

    this.config = {
      endpoint: config.endpoint,
      siteId: config.siteId,
      autoTrack: config.autoTrack ?? true,
      respectDNT: config.respectDNT ?? false,
      trackScroll: config.trackScroll ?? false,
      trackOutboundLinks: config.trackOutboundLinks ?? false,
    }

    if (this.config.respectDNT && checkDNT()) {
      this.stopped = true
      return
    }

    // Guard against duplicate instances on the same page (snippet pasted twice,
    // SPA bundle re-evaluated on hot reload, etc). Doubling counts is a common
    // and hard-to-debug source of inflated metrics.
    const w = globalThis as Record<string, unknown>
    if (w[INSTANCE_KEY]) {
      if (typeof console !== 'undefined') {
        console.warn(
          'pagetally: an Analytics instance is already running on this page; new instance disabled',
        )
      }
      this.stopped = true
      return
    }
    w[INSTANCE_KEY] = true
    this.cleanups.push(() => {
      delete w[INSTANCE_KEY]
    })

    if (this.config.autoTrack) {
      this._startAutoTracking()
    }

    this._startPerformanceTracking()
  }

  private _send(payload: Omit<Payload, 's'>): void {
    if (this.stopped) return
    sendPayload({ ...payload, s: this.config.siteId } as Payload, this.config.endpoint)
  }

  private _startAutoTracking(): void {
    const eng = startEngagement((path, dur) => {
      this._send(buildPageLeavePayload(path, dur))
    })
    this.engagement = eng
    this.cleanups.push(() => eng.stop())

    if (this.config.trackScroll) {
      const scroll = startScrollTracking((pct) => {
        this._send(buildEventPayload('scroll_depth', { pct }))
      })
      this.scroll = scroll
      this.cleanups.push(() => scroll.stop())
    }

    if (this.config.trackOutboundLinks) {
      const clicks = startClickTracking((name, props) => {
        this._send(buildEventPayload(name, props))
      })
      this.cleanups.push(() => clicks.stop())
    }

    const fireView = (path?: string) => {
      const next = path ?? getPath()
      const now = Date.now()
      if (next === this.lastViewPath && now - this.lastViewTime < 500) return
      this.lastViewPath = next
      this.lastViewTime = now
      eng.flush()
      this._send(buildPageViewPayload(next))
      eng.reset(next)
      this.scroll?.reset()
    }
    this.cleanups.push(startAutoTracking(() => fireView()))

    const onPageShow = (e: PageTransitionEvent) => {
      if (e.persisted) fireView()
    }
    window.addEventListener('pageshow', onPageShow)
    this.cleanups.push(() => window.removeEventListener('pageshow', onPageShow))

    // Speculation-rules / Chromium prerender loads the page invisibly. Firing
    // a pageview during prerender double-counts whenever the user never lands
    // on the prerendered URL. Defer the initial view until activation.
    const prerendering =
      (document as Document & { prerendering?: boolean }).prerendering === true
    if (prerendering) {
      const onActivate = () => {
        document.removeEventListener('prerenderingchange', onActivate)
        fireView()
      }
      document.addEventListener('prerenderingchange', onActivate)
      this.cleanups.push(() =>
        document.removeEventListener('prerenderingchange', onActivate),
      )
    } else {
      fireView()
    }
  }

  private _startPerformanceTracking(): void {
    const send = (metrics: PerformanceMetrics) => {
      this._send(buildPerformancePayload(metrics))
    }
    this.cleanups.push(startPerformanceTracking(send))
  }

  /** Track a custom event. */
  track(name: string, props?: Record<string, unknown>): void {
    if (this.stopped) return
    this._send(buildEventPayload(name, props))
  }

  /** Manually track a page view. */
  page(path?: string): void {
    if (this.stopped) return
    this.engagement?.flush()
    const next = path ?? getPath()
    this._send(buildPageViewPayload(next))
    this.engagement?.reset(next)
    this.scroll?.reset()
  }

  /** Stop all tracking and clean up observers. */
  stop(): void {
    this.engagement?.flush()
    this.stopped = true
    for (const cleanup of this.cleanups) {
      cleanup()
    }
    this.cleanups = []
    this.engagement = null
    this.scroll = null
  }
}
