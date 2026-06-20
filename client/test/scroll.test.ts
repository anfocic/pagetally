import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { startScrollTracking, type ScrollTracker } from '../src/scroll'

function setDims(scrollHeight: number, innerHeight: number, scrollY: number) {
  for (const prop of ['scrollHeight', 'offsetHeight'] as const) {
    Object.defineProperty(document.documentElement, prop, {
      value: scrollHeight,
      configurable: true,
    })
  }
  Object.defineProperty(window, 'innerHeight', { value: innerHeight, configurable: true })
  Object.defineProperty(window, 'scrollY', { value: scrollY, configurable: true })
}

function scrollTo(y: number) {
  Object.defineProperty(window, 'scrollY', { value: y, configurable: true })
  window.dispatchEvent(new Event('scroll'))
}

describe('startScrollTracking', () => {
  let tracker: ScrollTracker | null = null

  beforeEach(() => {
    // Run the rAF callback synchronously so each scroll measures immediately.
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      cb(0)
      return 0
    })
  })

  afterEach(() => {
    tracker?.stop()
    tracker = null
    vi.unstubAllGlobals()
  })

  it('fires each milestone once as the user scrolls down', () => {
    setDims(5000, 1000, 0) // 20% visible at top → nothing on start
    const fired: number[] = []
    tracker = startScrollTracking((pct) => fired.push(pct))
    expect(fired).toEqual([])

    scrollTo(250) // (250+1000)/5000 = 25%
    expect(fired).toEqual([25])
    scrollTo(1500) // 50%
    expect(fired).toEqual([25, 50])
    scrollTo(4000) // 100% → also passes 75
    expect(fired).toEqual([25, 50, 75, 100])
  })

  it('does not re-fire a milestone on scroll up then down', () => {
    setDims(5000, 1000, 0)
    const fired: number[] = []
    tracker = startScrollTracking((pct) => fired.push(pct))

    scrollTo(1500) // 25 + 50
    scrollTo(0) // back to top
    scrollTo(1500) // no new fires
    expect(fired).toEqual([25, 50])
  })

  it('fires all milestones on start for a fully visible short page', () => {
    setDims(800, 1000, 0) // viewport taller than page → 100%
    const fired: number[] = []
    tracker = startScrollTracking((pct) => fired.push(pct))
    expect(fired).toEqual([25, 50, 75, 100])
  })

  it('reset() allows milestones to fire again for a new page view', () => {
    setDims(5000, 1000, 0)
    const fired: number[] = []
    tracker = startScrollTracking((pct) => fired.push(pct))
    scrollTo(250)
    expect(fired).toEqual([25])

    tracker.reset()
    fired.length = 0
    scrollTo(250)
    expect(fired).toEqual([25])
  })

  it('emits nothing after stop()', () => {
    setDims(5000, 1000, 0)
    const fired: number[] = []
    tracker = startScrollTracking((pct) => fired.push(pct))
    tracker.stop()
    scrollTo(4000)
    expect(fired).toEqual([])
  })
})
