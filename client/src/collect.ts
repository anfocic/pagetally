export function startAutoTracking(onPageView: () => void): () => void {
  const pushState = history.pushState.bind(history)
  const replaceState = history.replaceState.bind(history)

  function patch<
    F extends (data: unknown, title: string, url?: string | URL | null) => void,
  >(original: F): F {
    return function (this: History, ...args: Parameters<F>) {
      const result = original.apply(this, args)
      onPageView()
      return result
    } as F
  }

  history.pushState = patch(pushState)
  history.replaceState = patch(replaceState)

  window.addEventListener('popstate', onPageView)
  window.addEventListener('hashchange', onPageView)

  return () => {
    history.pushState = pushState
    history.replaceState = replaceState
    window.removeEventListener('popstate', onPageView)
    window.removeEventListener('hashchange', onPageView)
  }
}
