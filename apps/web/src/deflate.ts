/** The DEFLATE steps the worker sends, as the player and the codes view read them. */

/** Numbers per step in a batch from the worker; see `Parsed::steps`. */
export const STRIDE = 6;

export const StepKind = { BlockStart: 0, Literal: 1, Match: 2, BlockEnd: 3, Failure: 4 } as const;
export const BLOCK_NAMES = ["stored", "fixed Huffman", "dynamic Huffman"];

export interface Step {
  index: number;
  kind: number;
  /** Block kind, literal byte or distance, by `kind`. */
  a: number;
  /** Match length. */
  b: number;
  bitStart: number;
  bitEnd: number;
  outStart: number;
}
