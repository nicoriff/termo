/**
 * The time, once a second, for the items that show one or count towards one
 * (the clock, "fetched 5 min ago", when a plan window starts over). Ticks
 * only while one of them holds it.
 */
class Ticker {
  now = $state(Date.now());

  private holders = 0;
  private timer: ReturnType<typeof setInterval> | null = null;

  hold(): () => void {
    this.holders += 1;
    if (!this.timer) {
      this.now = Date.now();
      this.timer = setInterval(() => (this.now = Date.now()), 1000);
    }
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.holders -= 1;
      if (this.holders > 0 || !this.timer) return;
      clearInterval(this.timer);
      this.timer = null;
    };
  }
}

export const ticker = new Ticker();
