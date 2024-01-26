import { fetchServerResponse } from '../fetch-server-response'
import type {
  PrefetchAction,
  ReducerState,
  ReadonlyReducerState,
} from '../router-reducer-types'
import { PrefetchKind } from '../router-reducer-types'
import { prunePrefetchCache } from './prune-prefetch-cache'
import { NEXT_RSC_UNION_QUERY } from '../../app-router-headers'
import { PromiseQueue } from '../../promise-queue'
import { createPrefetchCacheKey } from './create-prefetch-cache-key'

export const prefetchQueue = new PromiseQueue(5)

export function prefetchReducer(
  state: ReadonlyReducerState,
  action: PrefetchAction
): ReducerState {
  // let's prune the prefetch cache before we do anything else
  prunePrefetchCache(state.prefetchCache)

  const { url } = action
  url.searchParams.delete(NEXT_RSC_UNION_QUERY)

  let prefetchCacheKey = createPrefetchCacheKey(url)
  const cacheEntry = state.prefetchCache.get(prefetchCacheKey)

  if (cacheEntry) {
    /**
     * If the cache entry present was marked as temporary, it means that we prefetched it from the navigate reducer,
     * where we didn't have the prefetch intent. We want to update it to the new, more accurate, kind here.
     */
    if (cacheEntry.kind === PrefetchKind.TEMPORARY) {
      state.prefetchCache.set(prefetchCacheKey, {
        ...cacheEntry,
        kind: action.kind,
      })
    }

    /**
     * if the prefetch action was a full prefetch and that the current cache entry wasn't one, we want to re-prefetch,
     * otherwise we can re-use the current cache entry
     **/
    if (
      !(
        cacheEntry.kind === PrefetchKind.AUTO &&
        action.kind === PrefetchKind.FULL
      )
    ) {
      return state
    }
  }

  // fetchServerResponse is intentionally not awaited so that it can be unwrapped in the navigate-reducer
  const serverResponse = prefetchQueue.enqueue(async () => {
    const prefetchResponse = await fetchServerResponse(
      url,
      state.tree,
      state.nextUrl,
      state.buildId,
      action.kind
    )

    /* [flightData, canonicalUrlOverride, postpone, intercept] */
    const [, , , intercept] = prefetchResponse
    const existingPrefetchEntry = state.prefetchCache.get(prefetchCacheKey)
    // If we discover that the prefetch corresponds with an interception route, we want to move it to
    // a prefixed cache key to avoid clobbering an existing entry.
    if (intercept && existingPrefetchEntry) {
      const prefixedCacheKey = createPrefetchCacheKey(url, state.nextUrl)
      state.prefetchCache.set(prefixedCacheKey, existingPrefetchEntry)
      state.prefetchCache.delete(prefetchCacheKey)
    }

    return prefetchResponse
  })

  // Create new tree based on the flightSegmentPath and router state patch
  state.prefetchCache.set(prefetchCacheKey, {
    treeAtTimeOfPrefetch: state.tree,
    data: serverResponse,
    kind: action.kind,
    prefetchTime: Date.now(),
    lastUsedTime: null,
  })

  return state
}
