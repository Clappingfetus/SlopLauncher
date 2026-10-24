import { it, expect, vi } from "vitest";
import { ActivityLatch } from "./activity-latch";
it("keeps bursty activity steady until ten seconds of quiet and cancels timers on disposal", () => {
  vi.useFakeTimers();
  try {
    const changed = vi.fn();
    const activity = new ActivityLatch(changed);
    for (let i = 0; i < 20; i++) {
      activity.update(true);
      activity.update(false);
      vi.advanceTimersByTime(500);
    }
    expect(changed.mock.calls).toEqual([[true]]);
    vi.advanceTimersByTime(9500);
    expect(changed.mock.calls).toEqual([[true], [false]]);
    activity.update(true);
    activity.update(false);
    activity.dispose();
    vi.advanceTimersByTime(10000);
    expect(changed.mock.calls).toEqual([[true], [false], [true]]);
  } finally {
    vi.useRealTimers();
  }
});
