// "Write to the developer": a problem, an idea or thanks, sent from the page
// without an account. What goes is what the person sees in the dialog —
// their words, a way to answer if they give one, the open file's structure
// report if they tick it (its layout, none of its content) — and nothing
// else: not the file, not an address. Kept by server/feedback.ts.
import { dismissable } from "./dialogs";
import { currentLocale, translateText } from "./i18n";

declare const __VERSION__: string;

type Kind = "bug" | "idea" | "thanks";

const ISSUES = "https://github.com/MykytaStel/hexscope/issues/new/choose";

const PROMPTS: Record<Kind, string> = {
  bug: "What happened, and what you expected. Which file it was, in words — the file itself is never sent.",
  idea: "What would make hexscope more useful to you?",
  thanks: "A word for the developer.",
};

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** The browser and system in two words, as a bug report needs them: "Safari on iPhone". */
export function deviceName(ua: string): string {
  const browser = /Edg\//.test(ua)
    ? "Edge"
    : /Firefox\//.test(ua)
      ? "Firefox"
      : /OPR\//.test(ua)
        ? "Opera"
        : /Chrome\//.test(ua) || /CriOS\//.test(ua)
          ? "Chrome"
          : /Safari\//.test(ua)
            ? "Safari"
            : "a browser";
  const system = /iPhone/.test(ua)
    ? "iPhone"
    : /iPad/.test(ua)
      ? "iPad"
      : /Android/.test(ua)
        ? "Android"
        : /Mac OS X/.test(ua)
          ? "Mac"
          : /Windows/.test(ua)
            ? "Windows"
            : /Linux/.test(ua)
              ? "Linux"
              : "";
  return system ? `${browser} on ${system}` : browser;
}

/** Opens the dialog; `report` is the open file's structure report, offered with a box to tick. */
export function openFeedback(options: { kind?: Kind; report?: string } = {}): void {
  const dialog = el("dialog", "report feedback");
  const form = el("form", "feedback-form");
  form.noValidate = true;
  let kind: Kind = options.kind ?? "bug";

  const kinds = el("div", "feedback-kinds");
  kinds.setAttribute("role", "radiogroup");
  kinds.setAttribute("aria-label", "What it is about");
  const message = el("textarea", "feedback-message");
  message.rows = 6;
  message.maxLength = 4000;
  message.required = true;
  message.setAttribute("aria-label", translateText("Your message", currentLocale()));
  const choices: [Kind, string][] = [
    ["bug", "Something is wrong"],
    ["idea", "An idea"],
    ["thanks", "Thanks"],
  ];
  const buttons = choices.map(([k, label]) => {
    const b = el("button", "feedback-kind", label);
    b.type = "button";
    b.setAttribute("role", "radio");
    b.addEventListener("click", () => choose(k));
    kinds.append(b);
    return [k, b] as const;
  });
  const choose = (k: Kind) => {
    kind = k;
    for (const [key, b] of buttons) b.setAttribute("aria-checked", String(key === k));
    // A textarea is left alone by the page's translation, which keeps what people type: its prompt is translated here.
    message.placeholder = translateText(PROMPTS[k], currentLocale());
  };
  choose(kind);

  const contact = el("input", "feedback-contact");
  contact.type = "text";
  contact.maxLength = 200;
  contact.autocomplete = "email";
  contact.placeholder = "Email or Telegram, if you want an answer — optional";
  contact.setAttribute("aria-label", "How to answer you");

  // A field people never see, left empty by them and filled by robots.
  const trap = el("input", "feedback-trap");
  trap.name = "website";
  trap.tabIndex = -1;
  trap.autocomplete = "off";
  trap.setAttribute("aria-hidden", "true");

  const attach = el("label", "feedback-attach");
  const box = el("input");
  box.type = "checkbox";
  box.checked = Boolean(options.report);
  attach.append(box, el("span", undefined, "Attach the open file's structure — its layout, none of its content"));
  const preview = el("details", "feedback-preview");
  const shown = el("textarea", "report-text");
  shown.readOnly = true;
  shown.rows = 8;
  shown.value = options.report ?? "";
  preview.append(el("summary", undefined, "See exactly what is attached"), shown);

  const device = deviceName(navigator.userAgent);
  const version = typeof __VERSION__ === "string" ? __VERSION__ : "dev";
  const also = el("p", "hint", "Also sent: the version of hexscope, your browser and system in two words, and this page's address. Never your file, never your IP address.");
  const status = el("p", "feedback-status");
  status.setAttribute("role", "status");

  const send = el("button", "btn btn-primary", "Send");
  send.type = "submit";
  const cancel = el("button", "btn", "Cancel");
  cancel.type = "button";
  cancel.addEventListener("click", () => dialog.close());
  const actions = el("div", "report-actions");
  actions.append(cancel, send);

  form.append(kinds, message, contact, trap);
  if (options.report) form.append(attach, preview);
  form.append(also, status, actions);
  dialog.append(
    el("h2", undefined, "Write to the developer"),
    el("p", "hint", "hexscope is made by one person. A problem you met, an idea, a word of thanks — it is read, and it makes the next version better."),
    form,
  );

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (message.value.trim().length < 3) {
      status.textContent = "Write a few words first.";
      message.focus();
      return;
    }
    send.disabled = true;
    status.textContent = "Sending…";
    try {
      const res = await fetch("/api/feedback", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "omit",
        body: JSON.stringify({
          kind,
          message: message.value,
          contact: contact.value,
          report: options.report && box.checked ? options.report : "",
          page: location.pathname,
          lang: currentLocale(),
          version,
          device,
          website: trap.value,
        }),
      });
      if (!res.ok) throw new Error(res.status === 429 ? "Too many messages today. Try again tomorrow." : "The site could not take it right now.");
      form.replaceChildren(
        el("p", "feedback-done", kind === "thanks" ? "Sent. Thank you — it means a lot." : "Sent. Thank you — it will be read."),
        Object.assign(el("button", "btn btn-primary", "Close"), { type: "button", onclick: () => dialog.close() }),
      );
    } catch (err) {
      send.disabled = false;
      const why = err instanceof Error && err.message.endsWith(".") ? err.message : "There is no connection.";
      status.replaceChildren(
        el("span", undefined, "Not sent."),
        document.createTextNode(" "),
        el("span", undefined, why),
        document.createTextNode(" "),
        Object.assign(el("a", undefined, "Write on GitHub instead ↗"), { href: ISSUES, target: "_blank", rel: "noopener noreferrer" }),
      );
    }
  });

  dialog.addEventListener("close", () => dialog.remove());
  document.body.append(dialog);
  dismissable(dialog, true);
  dialog.showModal();
  message.focus();
}
