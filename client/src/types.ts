export interface AnalyticsConfig {
  endpoint: string
  siteId: string
  autoTrack?: boolean
  respectDNT?: boolean
}

export type EventType = 'pageview' | 'event' | 'performance' | 'pageleave'

export type PerformanceMetrics = {
  lcp?: number
  fcp?: number
  cls?: number
  inp?: number
  ttfb?: number
}

export interface Payload {
  t: EventType
  s: string
  p: string
  ts: number
  r?: string
  d?: 'mobile' | 'tablet' | 'desktop'
  v?: number
  n?: string
  pr?: Record<string, unknown>
  pf?: PerformanceMetrics
  dur?: number
}
