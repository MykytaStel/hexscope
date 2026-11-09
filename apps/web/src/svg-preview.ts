import { el } from "./dom";
import { currentLocale, translateText } from "./i18n";

const MAX_BYTES = 2 * 1024 * 1024;
const GZIP = Uint8Array.of(0x1f, 0x8b);
const tr = (text: string) => translateText(text, currentLocale());
const UNSAFE_ELEMENTS = new Set([
  "script", "style", "foreignobject", "iframe", "object", "embed", "audio", "video",
  "animate", "animatemotion", "animatetransform", "set",
]);
const SAFE_IMAGE_DATA = /^data:image\/(?:png|jpeg|webp|gif);base64,[a-z0-9+/=]+$/i;
const INTERNAL_URL = /^url\(\s*['"]?#[\w:.-]+['"]?\s*\)$/i;

function isGzip(bytes: Uint8Array): boolean {
  return bytes.length >= 2 && bytes[0] === GZIP[0] && bytes[1] === GZIP[1];
}

function safeSvg(bytes: Uint8Array): string {
  const source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  if (/<!doctype/i.test(source)) throw new Error("SVG with a document type declaration is not previewed.");
  const parsed = new DOMParser().parseFromString(source, "image/svg+xml");
  if (parsed.querySelector("parsererror") || parsed.documentElement.localName.toLowerCase() !== "svg") {
    throw new Error("This file could not be read as SVG XML.");
  }

  for (const node of Array.from(parsed.childNodes)) {
    if (node.nodeType === Node.PROCESSING_INSTRUCTION_NODE) node.remove();
  }
  for (const node of Array.from(parsed.querySelectorAll("*"))) {
    if (UNSAFE_ELEMENTS.has(node.localName.toLowerCase())) {
      node.remove();
      continue;
    }
    for (const attribute of Array.from(node.attributes)) {
      const name = attribute.localName.toLowerCase();
      const value = attribute.value.trim();
      if (name.startsWith("on") || name === "style" || name === "src" || name === "base") {
        node.removeAttributeNode(attribute);
      } else if (name === "href") {
        if (value.startsWith("#") || (value.length <= 350_000 && SAFE_IMAGE_DATA.test(value))) continue;
        node.removeAttributeNode(attribute);
      } else if (/url\s*\(/i.test(value) && !INTERNAL_URL.test(value)) {
        node.removeAttributeNode(attribute);
      }
    }
  }
  return new XMLSerializer().serializeToString(parsed);
}

/** Makes a clicked, size-bounded SVG preview after removing active/external content. */
export function svgStaticPreview(bytes: Uint8Array): HTMLElement {
  const box = el("section", "svg-static-preview");
  box.append(el("h4", undefined, tr("Static SVG preview")));
  const status = el("p", "hint svg-preview-status", tr("Scripts, event handlers, embedded HTML, external links and CSS are removed from this local preview."));
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  const frame = el("div", "svg-preview-frame");
  const button = el("button", "link svg-preview-button", tr("Make static preview"));
  button.type = "button";
  box.append(status, button, frame);

  if (isGzip(bytes)) {
    button.disabled = true;
    status.textContent = tr("Compressed SVG files are not shown in this preview.");
  } else if (bytes.length > MAX_BYTES) {
    button.disabled = true;
    status.textContent = tr("Static preview is limited to SVG files no larger than 2 MiB.");
  }

  button.addEventListener("click", () => {
    button.disabled = true;
    status.textContent = tr("Preparing a local static preview…");
    frame.replaceChildren();
    try {
      const data = safeSvg(bytes);
      const url = URL.createObjectURL(new Blob([data], { type: "image/svg+xml" }));
      const image = el("img", "svg-preview-image");
      image.alt = tr("Sanitized static preview of this SVG drawing");
      const release = () => URL.revokeObjectURL(url);
      image.addEventListener("load", release, { once: true });
      image.addEventListener("error", release, { once: true });
      image.src = url;
      frame.append(image);
      status.textContent = tr("This preview is a sanitized still; it does not show external or active content.");
    } catch (error) {
      status.textContent = error instanceof Error ? tr(error.message) : tr("This SVG could not be previewed safely.");
      button.disabled = false;
    }
  });
  return box;
}
