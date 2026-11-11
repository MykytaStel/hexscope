import { parseAside } from "./compare";

/** Only the shipped synthetic PNG is mutated; the user's source never enters. */
export async function runDamageLab(): Promise<{ before: number; after: number }> {
  const response = await fetch(new URL("samples/sample.png", document.baseURI));
  if (!response.ok) throw new Error("Sample unavailable offline");
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length < 33 || bytes.length > 4 * 1024 * 1024 || bytes[0] !== 137 || bytes[1] !== 80) throw new Error("Unexpected sample");
  const original = await parseAside(new File([bytes], "synthetic-before.png"));
  bytes[29] ^= 1; // IHDR CRC, never image content or an uploaded input.
  const changed = await parseAside(new File([bytes], "synthetic-after.png"));
  return { before: original.problems.length, after: changed.problems.length };
}
