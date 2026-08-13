/**
 * Trailing-edge debounce.
 *
 * The preview pane runs at ~150 ms. That number is a compromise:
 * short enough that the pane feels like it is following your cursor, long
 * enough that a fast typist does not queue a generation per character. The
 * trailing edge matters — you want the state after the burst, not before it.
 */
export function debounce<A extends unknown[]>(
  fn: (...args: A) => void,
  ms = 150
): ((...args: A) => void) & { cancel: () => void; flush: (...args: A) => void } {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const wrapped = (...args: A) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, ms);
  };

  wrapped.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };

  wrapped.flush = (...args: A) => {
    wrapped.cancel();
    fn(...args);
  };

  return wrapped;
}

/** Bytes, for artifact sizes in the UI. */
export function humanBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KiB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MiB`;
}
