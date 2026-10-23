import type { Family } from "./family";

/** One lazy WASM loader per parser family, kept out of the worker coordinator. */
export const PARSER_BUILDS: Record<Family, () => Promise<unknown>> = {
  media: () => import("./wasm-media/hexscope_media.js"),
  pdf: () => import("./wasm-pdf/hexscope_pdf.js"),
  docs: () => import("./wasm-docs/hexscope_docs.js"),
  audio: () => import("./wasm-audio/hexscope_audio.js"),
};
