import { useEffect, useRef } from "react";

/** Base gap between status polls; every poller uses this one constant. */
export const POLL_MS = 4000;
const MAX_BACKOFF = 4;

/** Calls `tick` one at a time (never overlapping), only while the window is visible. `tick` returns a fingerprint of what it
 *  loaded; while the fingerprint stays the same the gap grows from 1x to 4x POLL_MS, and any change or showing the window resets it. */
export function usePoll(tick: () => Promise<string>) {
  const latest = useRef(tick);
  useEffect(() => { latest.current = tick; });
  useEffect(() => {
    let timer: number | undefined;
    let stopped = false;
    let busy = false;
    let last = "";
    let same = 0;
    const schedule = () => {
      if (!stopped) timer = window.setTimeout(run, POLL_MS * Math.min(1 + same, MAX_BACKOFF));
    };
    const run = async () => {
      if (busy) return;
      if (document.visibilityState === "visible") {
        busy = true;
        try {
          const fp = await latest.current();
          same = fp === last ? same + 1 : 0;
          last = fp;
        } catch {
          // keep polling; the caller shows its own errors
        }
        busy = false;
      }
      schedule();
    };
    const onVisible = () => {
      if (document.visibilityState !== "visible") return;
      window.clearTimeout(timer);
      same = 0;
      void run();
    };
    document.addEventListener("visibilitychange", onVisible);
    void run();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, []);
}
