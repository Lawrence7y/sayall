import { afterEach, describe, expect, it, vi } from "vitest";
import { startVisiblePoll } from "./visible-poll";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("visible page polling", () => {
  it("pauses while hidden, refreshes on return and cleans up", async () => {
    vi.useFakeTimers();
    let hidden = false;
    vi.spyOn(document, "hidden", "get").mockImplementation(() => hidden);
    const refresh = vi.fn(async () => {});
    const stop = startVisiblePoll(refresh, 1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(refresh).toHaveBeenCalledTimes(1);
    hidden = true;
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.advanceTimersByTimeAsync(5000);
    expect(refresh).toHaveBeenCalledTimes(1);
    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
    await vi.advanceTimersByTimeAsync(0);
    expect(refresh).toHaveBeenCalledTimes(2);
    stop();
    await vi.advanceTimersByTimeAsync(2000);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(refresh).toHaveBeenCalledTimes(2);
  });

  it("does not overlap slow requests or restart after disposal", async () => {
    vi.useFakeTimers();
    vi.spyOn(document, "hidden", "get").mockReturnValue(false);
    let resolve!: () => void;
    const refresh = vi.fn(() => new Promise<void>((done) => { resolve = done; }));
    const stop = startVisiblePoll(refresh, 1000);
    await vi.advanceTimersByTimeAsync(5000);
    expect(refresh).toHaveBeenCalledTimes(1);
    stop();
    resolve();
    await vi.advanceTimersByTimeAsync(5000);
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
