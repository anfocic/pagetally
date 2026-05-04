import type { AnalyticsConfig, Payload, PerformanceMetrics } from './types'
import { checkDNT } from './privacy'
import { sendPayload } from './transport'
import {
  buildPageViewPayload,
  buildEventPayload,
  buildPerformancePayload,
} from './payload'
import { startAutoTracking } from './collect'
import { startPerformanceTracking } from './performance'

export type { AnalyticsConfig, Payload, PerformanceMetrics } from './types'

export class Analytics {
  private config: Required<AnalyticsConfig>
  private cleanups: (() => void)[] = []
  private stopped = false

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
    const send = () => this._send(buildPageViewPayload())
    this.cleanups.push(startAutoTracking(send))
    send()
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
    this._send(buildPageViewPayload(path))
  }

  /** Stop all tracking and clean up observers. */
  stop(): void {
    this.stopped = true
    for (const cleanup of this.cleanups) {
      cleanup()
    }
    this.cleanups = []
  }
}
