// Only `pushState` is treated as a navigation. `replaceState` is intentionally
// not patched: SPA frameworks (Astro view transitions, React Router, Next,
// SvelteKit, Vue Router) call it during hydration/state-sync to normalize URLs
// without a real navigation, which would otherwise double-count the initial view.
export function startAutoTracking(onPageView: () => void): () => void {
  const pushState = history.pushState.bind(history)

  history.pushState = function (this: History, ...args: Parameters<typeof pushState>) {
    const result = pushState.apply(this, args)
    onPageView()
    return result
  }

  window.addEventListener('popstate', onPageView)
  window.addEventListener('hashchange', onPageView)

  return () => {
    history.pushState = pushState
    window.removeEventListener('popstate', onPageView)
    window.removeEventListener('hashchange', onPageView)
  }
}
