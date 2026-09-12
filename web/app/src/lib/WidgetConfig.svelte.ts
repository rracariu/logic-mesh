import { untrack } from 'svelte';
import type { Widget } from './Block';
import { onValue } from './UiConnector';
import { isHaystackNumber, numericValue } from './utils';

// Engine-published numbers arrive as primitives or Haystack scalar
// records; unwrap the latter so consumers see plain values.
function coerce(value: unknown): unknown {
  return isHaystackNumber(value) ? numericValue(value) : value;
}

/**
 * Effective config for a widget: the literal `config` values are the
 * defaults, and each `configSources` entry (config key → plain
 * ExternalOut address) overrides its key with the latest value the
 * engine publishes to that address. Call during component init — the
 * subscriptions live in an $effect and are cleaned up on destroy and
 * whenever the sources change.
 */
export function useWidgetConfig(getWidget: () => Widget | undefined) {
  let overrides = $state<Record<string, unknown>>({});

  $effect(() => {
    const sources = getWidget()?.configSources;
    const next: Record<string, unknown> = {};
    overrides = next;
    if (!sources) return;
    // Read the entries here (not inside untrack) so in-place per-key
    // edits of configSources resubscribe.
    const entries = Object.entries(sources).filter(([, address]) => address);
    // untrack: onValue replays a cached publish synchronously, and
    // that callback must not register reads as effect dependencies.
    const unsubs = untrack(() =>
      entries.map(([key, address]) =>
        onValue(address, (value) => {
          const coerced = coerce(value);
          // A null or non-finite publish must not shadow the literal
          // config value; drop the override instead.
          if (
            coerced == null ||
            (typeof coerced === 'number' && !Number.isFinite(coerced))
          ) {
            if (!(key in next)) return;
            delete next[key];
          } else {
            // Change-of-value gate: an unchanged publish must not mint
            // a new config identity (downstream $deriveds would churn).
            if (Object.is(next[key], coerced)) return;
            next[key] = coerced;
          }
          overrides = { ...next };
        }),
      ),
    );
    return () => {
      for (const unsub of unsubs) unsub();
    };
  });

  const config = $derived({ ...(getWidget()?.config ?? {}), ...overrides });

  return {
    get config() {
      return config;
    },
  };
}

/**
 * Subscribes an input widget to its `valueSource` — the address of a
 * plain ExternalOut block whose published values the widget tracks as
 * feedback. Values are coerced like config sources and handed to
 * `onFeedback`, which must never re-push them to the engine.
 *
 * When `isInteracting` is given, feedback arriving while it is true is
 * deferred rather than dropped: the last suppressed value is applied
 * when the interaction ends — unless the user edited during it (call
 * `markEdited()` from the widget's push handlers), in which case it is
 * discarded and the next publish re-syncs. Call during component init.
 */
export function useValueFeedback(
  getWidget: () => Widget | undefined,
  onFeedback: (value: unknown) => void,
  isInteracting?: () => boolean,
) {
  let pending: unknown;
  let hasPending = false;
  let edited = false;

  $effect(() => {
    const address = getWidget()?.valueSource;
    hasPending = false;
    pending = undefined;
    if (!address) return;
    // Deliberate: the cached publish replays here before the widget's
    // mount push, so a pasted widget pushes the tracked value.
    return untrack(() =>
      onValue(address, (value) => {
        if (isInteracting?.()) {
          pending = coerce(value);
          hasPending = true;
          return;
        }
        onFeedback(coerce(value));
      }),
    );
  });

  if (isInteracting) {
    $effect(() => {
      if (isInteracting()) {
        edited = false;
        return;
      }
      if (hasPending) {
        if (!edited) untrack(() => onFeedback(pending));
        pending = undefined;
        hasPending = false;
      }
      edited = false;
    });
  }

  return {
    /** The user edited during this interaction — their push wins. */
    markEdited() {
      edited = true;
    },
  };
}
