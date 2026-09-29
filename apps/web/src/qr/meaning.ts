// What a QR code's text is: a link and where it really goes, a Wi-Fi
// network and its password, a two-factor secret, a contact, a place.

/** Short-link services: where one ends up is not in the link. */
const SHORTENERS = [
  "bit.ly", "t.co", "tinyurl.com", "goo.gl", "ow.ly", "buff.ly", "is.gd", "rb.gy", "cutt.ly", "shorturl.at", "s.id",
  "t.ly", "tiny.cc", "rebrand.ly", "qrco.de", "qr.link", "lnkd.in", "bl.ink", "short.io", "surl.li",
];

/** One label of an internationalised domain name, from its `xn--` form (RFC 3492). */
function punycode(label: string): string {
  const input = label.slice(4);
  const dash = input.lastIndexOf("-");
  const out = dash > 0 ? [...input.slice(0, dash)].map((c) => c.codePointAt(0)!) : [];
  let n = 128;
  let bias = 72;
  let i = 0;
  for (let at = dash > 0 ? dash + 1 : 0; at < input.length; ) {
    const old = i;
    for (let w = 1, k = 36; ; k += 36) {
      if (at >= input.length) return label;
      const c = input.charCodeAt(at++);
      const digit = c >= 48 && c <= 57 ? c - 22 : c >= 65 && c <= 90 ? c - 65 : c >= 97 && c <= 122 ? c - 97 : -1;
      if (digit < 0) return label;
      i += digit * w;
      const t = k <= bias ? 1 : k >= bias + 26 ? 26 : k - bias;
      if (digit < t) break;
      w *= 36 - t;
    }
    const len = out.length + 1;
    let delta = old === 0 ? Math.floor((i - old) / 700) : Math.floor((i - old) / 2);
    delta += Math.floor(delta / len);
    let k = 0;
    for (; delta > 455; k += 36) delta = Math.floor(delta / 35);
    bias = k + Math.floor((36 * delta) / (delta + 38));
    n += Math.floor(i / len);
    i %= len;
    out.splice(i++, 0, n);
  }
  return String.fromCodePoint(...out);
}

/** A `KEY:value;` field of a Wi-Fi or contact code, with its escapes undone. */
function field(text: string, key: string): string {
  const m = new RegExp(`(?:^|[:;])${key}:((?:\\\\.|[^;])*)`, "i").exec(text);
  return m ? m[1].replace(/\\(.)/g, "$1") : "";
}

const quote = (s: string, max = 120) => `“${s.length > max ? `${s.slice(0, max)}…` : s}”`;

function safeUrl(t: string): URL | null {
  try {
    return new URL(t);
  } catch {
    return null;
  }
}

/** A link, and what about it is meant to mislead. */
function link(url: URL, raw: string): { kind: string; text: string } {
  const host = url.hostname;
  const shown = host.split(".").map((l) => (l.startsWith("xn--") ? punycode(l) : l)).join(".");
  const tricks: string[] = [];
  if (shown !== host) tricks.push(`shown as ${quote(shown)}: letters of other alphabets that pass for Latin ones`);
  // `https://bank.com@evil.example/` goes to evil.example.
  const authority = raw.replace(/^https?:\/\//i, "").split(/[/?#]/)[0];
  if (authority.includes("@")) tricks.push(`the part before the @, ${quote(authority.split("@")[0], 40)}, is not where it goes`);
  if (/^\d+\.\d+\.\d+\.\d+$/.test(host) || host.startsWith("[")) tricks.push("an address in numbers, not a site's name");
  const short = SHORTENERS.includes(host.replace(/^www\./, "")) ? ", a short link that does not say where it ends up" : "";
  const where = `goes to ${host}${short}: ${quote(raw, 160)}`;
  return tricks.length ? { kind: "qrtrick", text: `${where} — ${tricks.join("; ")}` } : { kind: "qrlink", text: where };
}

/** What a code's text is, in the kind of fact it makes. */
export function meaning(text: string): { kind: string; text: string } {
  const t = text.trim();
  if (/^WIFI:/i.test(t)) {
    const name = field(t, "S");
    const password = field(t, "P");
    return password
      ? { kind: "qrwifi", text: `the Wi-Fi network ${quote(name)} and its password ${quote(password)}` }
      : { kind: "qrwifi", text: `the open Wi-Fi network ${quote(name)}` };
  }
  if (/^otpauth:\/\//i.test(t)) {
    const url = safeUrl(t);
    // otpauth://totp/Issuer:account?secret=…
    const path = decodeURIComponent(url?.pathname.replace(/^\/+/, "") ?? "");
    const label = path.slice(path.indexOf(":") + 1).trim();
    const issuer = url?.searchParams.get("issuer");
    return { kind: "qrsecret", text: `the two-factor secret${issuer ? ` for ${issuer}` : ""}${label ? `, ${quote(label)}` : ""}: whoever has it can make the sign-in codes` };
  }
  if (/^geo:/i.test(t)) {
    const [lat, lon] = t.slice(4).split(/[,;?]/);
    return { kind: "qrplace", text: `the place ${lat}, ${lon}` };
  }
  if (/^BEGIN:VCARD/i.test(t) || /^MECARD:/i.test(t)) {
    const vcard = /^BEGIN:VCARD/i.test(t);
    const line = (key: string) => (vcard ? (new RegExp(`^${key}[^:\\n]*:(.*)$`, "im").exec(t)?.[1] ?? "").trim() : field(t, key));
    const parts = [line(vcard ? "FN" : "N"), line("TEL"), line("EMAIL")].filter(Boolean);
    return { kind: "qrcontact", text: `a contact card: ${parts.join(", ") || "a name and numbers"}` };
  }
  if (/^(mailto|tel|sms|smsto):/i.test(t)) {
    const [scheme, rest] = [t.slice(0, t.indexOf(":")).toLowerCase(), t.slice(t.indexOf(":") + 1)];
    const to = decodeURIComponent(rest.split(/[?:]/)[0]);
    const what = scheme === "mailto" ? "an email to" : scheme === "tel" ? "a call to" : "a text message to";
    return { kind: "qrtext", text: `starts ${what} ${to}` };
  }
  const url = /^https?:\/\//i.test(t) ? safeUrl(t) : null;
  if (url) return link(url, t);
  return { kind: "qrtext", text: `the text ${quote(t)}` };
}
