// What a person sends from "Report a problem" or "Send an idea": their words,
// how to answer them if they leave a way, and — only if they tick the box —
// the open file's structure report, which has its layout and none of its
// content. No address, no cookie, no file. Read by the site's author alone:
// `pnpm feedback` (scripts/feedback.mjs).

export const KINDS = ["bug", "idea", "thanks"] as const;
export type Kind = (typeof KINDS)[number];

/** Limits on what one message may carry, in characters. */
export const LIMITS = { message: 4000, contact: 200, report: 40_000, device: 120 } as const;
/** Messages taken a day, all senders together: enough for people, not for a flood. */
export const DAILY = 300;

export interface Feedback {
  kind: Kind;
  message: string;
  contact: string;
  report: string;
  page: string;
  lang: "en" | "uk";
  version: string;
  device: string;
}

export type Checked = { ok: true; feedback: Feedback } | { ok: false; status: 400 | 403 | 413 | 415; reason: string };

const text = (v: unknown, max: number) =>
  typeof v === "string" ? v.replace(/\r\n?/g, "\n").replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, "").trim().slice(0, max) : "";

/**
 * Checks a message before it is kept: from this site's own page, as JSON,
 * small, with something said, and the empty field robots fill left empty.
 */
export function check(input: { fetchSite: string | null; contentType: string | null; length: number; body: unknown }): Checked {
  if (input.fetchSite !== null && input.fetchSite !== "same-origin") return { ok: false, status: 403, reason: "not from this site" };
  if (!input.contentType?.startsWith("application/json")) return { ok: false, status: 415, reason: "JSON only" };
  if (input.length > 64 * 1024) return { ok: false, status: 413, reason: "too large" };
  const b = (input.body ?? {}) as Record<string, unknown>;
  // A field people never see: whatever fills it is not a person.
  if (typeof b.website === "string" && b.website !== "") return { ok: false, status: 400, reason: "not a person" };
  const kind = KINDS.includes(b.kind as Kind) ? (b.kind as Kind) : null;
  const message = text(b.message, LIMITS.message);
  if (!kind || message.length < 3) return { ok: false, status: 400, reason: "nothing said" };
  const page = text(b.page, 120);
  return {
    ok: true,
    feedback: {
      kind,
      message,
      contact: text(b.contact, LIMITS.contact),
      report: text(b.report, LIMITS.report),
      page: /^\/[a-z0-9/-]{0,100}$/.test(page) ? page : "",
      lang: b.lang === "uk" ? "uk" : "en",
      version: /^[0-9a-z.+-]{1,40}$/i.test(String(b.version ?? "")) ? String(b.version) : "",
      device: text(b.device, LIMITS.device),
    },
  };
}
