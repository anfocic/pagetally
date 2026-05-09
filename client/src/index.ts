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

export type { AnalyticsConfig, Payload, PerformanceMetrics } from './types'

export class Analytics {
  private config: Required<AnalyticsConfig>
  private cleanups: (() => void)[] = []
  private stopped = false
  private engagement: Engagement | null = null

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
    }

    if (this.config.respectDNT && checkDNT()) {
      this.stopped = true
      return
    }

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

    const fireView = (path?: string) => {
      eng.flush()
      const next = path ?? getPath()
      this._send(buildPageViewPayload(next))
      eng.reset(next)
    }
    this.cleanups.push(startAutoTracking(() => fireView()))

    const onPageShow = (e: PageTransitionEvent) => {
      if (e.persisted) fireView()
    }
    window.addEventListener('pageshow', onPageShow)
    this.cleanups.push(() => window.removeEventListener('pageshow', onPageShow))

    fireView()
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
  }
}
