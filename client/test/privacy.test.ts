import { describe, it, expect, beforeEach } from 'vitest'
import {
  stripQueryParams,
  getReferrerDomain,
  getDeviceClass,
  roundViewportWidth,
  checkDNT,
} from '../src/privacy'

describe('stripQueryParams', () => {
  it('returns path unchanged when no query string', () => {
    expect(stripQueryParams('/about')).toBe('/about')
  })

  it('strips query string', () => {
    expect(stripQueryParams('/about?utm=123')).toBe('/about')
  })

  it('handles empty string', () => {
    expect(stripQueryParams('')).toBe('')
  })

  it('handles only query string', () => {
    expect(stripQueryParams('?foo=bar')).toBe('')
  })
})

describe('getReferrerDomain', () => {
  it('returns hostname from referrer', () => {
    Object.defineProperty(document, 'referrer', {
      value: 'https://twitter.com/some/post',
      configurable: true,
    })
    expect(getReferrerDomain()).toBe('twitter.com')
  })

  it('returns undefined for empty referrer', () => {
    Object.defineProperty(document, 'referrer', {
      value: '',
      configurable: true,
    })
    expect(getReferrerDomain()).toBeUndefined()
  })

  it('returns undefined when referrer is invalid', () => {
    Object.defineProperty(document, 'referrer', {
      value: 'not-a-url',
      configurable: true,
    })
    expect(getReferrerDomain()).toBeUndefined()
  })
})

describe('getDeviceClass', () => {
  it('returns mobile for width < 640', () => {
    expect(getDeviceClass(320)).toBe('mobile')
    expect(getDeviceClass(639)).toBe('mobile')
  })

  it('returns tablet for width 640-1023', () => {
    expect(getDeviceClass(640)).toBe('tablet')
    expect(getDeviceClass(1023)).toBe('tablet')
  })

  it('returns desktop for width >= 1024', () => {
    expect(getDeviceClass(1024)).toBe('desktop')
    expect(getDeviceClass(1920)).toBe('desktop')
  })
})

describe('roundViewportWidth', () => {
  it('rounds to nearest 10', () => {
    expect(roundViewportWidth(1440)).toBe(1440)
    expect(roundViewportWidth(1443)).toBe(1440)
    expect(roundViewportWidth(1447)).toBe(1450)
  })

  it('handles zero', () => {
    expect(roundViewportWidth(0)).toBe(0)
  })
})

describe('checkDNT', () => {
  beforeEach(() => {
    Object.defineProperty(navigator, 'doNotTrack', {
      value: null,
      configurable: true,
    })
    Object.defineProperty(window, 'doNotTrack', {
      value: null,
      configurable: true,
    })
  })

  it('returns false when doNotTrack is not "1"', () => {
    Object.defineProperty(navigator, 'doNotTrack', {
      value: '0',
      configurable: true,
    })
    expect(checkDNT()).toBe(false)
  })

  it('returns true when doNotTrack is "1"', () => {
    Object.defineProperty(navigator, 'doNotTrack', {
      value: '1',
      configurable: true,
    })
    expect(checkDNT()).toBe(true)
  })

  it('returns false when doNotTrack is null', () => {
    expect(checkDNT()).toBe(false)
  })
})
