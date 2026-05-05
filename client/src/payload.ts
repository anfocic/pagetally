import type { Payload, PerformanceMetrics } from './types'
import {
  stripQueryParams,
  getReferrerDomain,
  getDeviceClass,
  roundViewportWidth,
} from './privacy'

export function getPath(): string {
  return stripQueryParams(location.pathname + location.search)
}

export function buildPageViewPayload(path?: string): Omit<Payload, 's'> {
  const w = roundViewportWidth(window.innerWidth)

  return {
    t: 'pageview',
    p: path ? stripQueryParams(path) : getPath(),
    ts: Date.now(),
    r: getReferrerDomain(),
    d: getDeviceClass(window.innerWidth),
    v: w > 0 ? w : undefined,
  }
}

export function buildEventPayload(
  name: string,
  props?: Record<string, unknown>,
): Omit<Payload, 's'> {
  return {
    t: 'event',
    p: getPath(),
    ts: Date.now(),
    n: name,
    ...(props && Object.keys(props).length > 0 ? { pr: props } : {}),
  }
}

export function buildPerformancePayload(metrics: PerformanceMetrics): Omit<Payload, 's'> {
  return {
    t: 'performance',
    p: getPath(),
    ts: Date.now(),
    pf: metrics,
  }
}

export function buildPageLeavePayload(path: string, dur: number): Omit<Payload, 's'> {
  return {
    t: 'pageleave',
    p: stripQueryParams(path),
    ts: Date.now(),
    dur,
  }
}
