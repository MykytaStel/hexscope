// Counts how many people visit, never who: see server/count.ts. Three empty
// requests, each kept by the browser until its period ends, so most visits
// send nothing at all. Opening a file sends nothing, ever.

/** Only the real site counts: not a preview, not a copy, not a test run. */
const COUNTED = new Set(["hexscope.org"]);

export function countVisit(): void {
  if (!COUNTED.has(location.hostname)) return;
  const nav = navigator as Navigator & { globalPrivacyControl?: boolean };
  if (nav.globalPrivacyControl || nav.doNotTrack === "1") return;
  // A browser a program drives — a crawler, a test, a page checker — is no person.
  if (nav.webdriver) return;
  const later = window.requestIdleCallback ?? ((run: () => void) => setTimeout(run, 2000));
  later(() => {
    for (const period of ["day", "month", "new"]) {
      // Details go only with the day's first visit, as headers: the address
      // stays the same, so the browser's kept answer covers every page.
      const headers = period === "day" ? about() : undefined;
      fetch(`/api/seen/${period}`, { headers, credentials: "omit", priority: "low" } as RequestInit).catch(() => {});
    }
  });
}

function about(): Record<string, string> {
  const coarse = matchMedia("(pointer: coarse)").matches;
  const device = !coarse ? "computer" : Math.min(screen.width, screen.height) >= 600 ? "tablet" : "phone";
  const out: Record<string, string> = {
    // The page not found says so, rather than whatever address was mistyped.
    "X-Seen-Page": document.body.dataset.page ?? location.pathname,
    "X-Seen-Device": device,
    "X-Seen-Lang": document.documentElement.lang === "uk" ? "uk" : "en",
  };
  try {
    const from = document.referrer ? new URL(document.referrer) : null;
    if (from && (from.protocol === "https:" || from.protocol === "http:") && from.hostname !== location.hostname) out["X-Seen-From"] = from.hostname;
  } catch {
    // An unreadable referrer is simply not sent.
  }
  return out;
}
