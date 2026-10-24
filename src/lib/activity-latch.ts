// Coalesce a burst of short requests into one calm activity session.
// Live counts can still be shown in tooltips without flashing the main status.
export class ActivityLatch {
  private active = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  constructor(
    private changed: (active: boolean) => void,
    private quietMs = 10_000,
  ) {}
  update(busy: boolean) {
    if (busy) {
      clearTimeout(this.timer);
      this.timer = undefined;
      if (!this.active) {
        this.active = true;
        this.changed(true);
      }
    } else if (this.active && this.timer === undefined) {
      this.timer = setTimeout(() => {
        this.timer = undefined;
        this.active = false;
        this.changed(false);
      }, this.quietMs);
    }
  }
  dispose() {
    clearTimeout(this.timer);
  }
}
