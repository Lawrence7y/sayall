/** Poll visible pages without overlapping requests; dispose safely during async work. */
export function startVisiblePoll(refresh: () => Promise<unknown>, intervalMs: number): () => void {
  let disposed = false;
  let running = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const schedule = () => {
    if (!disposed && !document.hidden && !running) timer = setTimeout(tick, intervalMs);
  };
  const tick = async () => {
    timer = undefined;
    if (disposed || document.hidden || running) return;
    running = true;
    try {
      await refresh();
    } catch {
      // Callers report request errors; a transient rejection must not stop polling.
    } finally {
      running = false;
      schedule();
    }
  };
  const onVisibilityChange = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    if (!document.hidden) void tick();
  };
  document.addEventListener("visibilitychange", onVisibilityChange);
  schedule();
  return () => {
    disposed = true;
    if (timer !== undefined) clearTimeout(timer);
    document.removeEventListener("visibilitychange", onVisibilityChange);
  };
}
