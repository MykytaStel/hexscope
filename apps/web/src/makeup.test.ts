import { expect, it } from "vitest";
import { barParts, MAX_BAR_PARTS } from "./makeup";
import { Role, type Slice } from "./model";

const slice = (start: number, len: number, role: number, node: number): Slice => ({ start, len, role, node });

it("keeps every slice of an ordinary file as its own part", () => {
  const slices = [slice(0, 2, Role.Structure, 1), slice(2, 20, Role.Metadata, 2), slice(22, 60, Role.Structure, 3), slice(82, 9, Role.Structure, 4), slice(91, 900, Role.Content, 5)];
  expect(barParts(slices)).toEqual([
    { role: Role.Structure, len: 2, node: 1, merged: false },
    { role: Role.Metadata, len: 20, node: 2, merged: false },
    { role: Role.Structure, len: 60, node: 3, merged: false },
    { role: Role.Structure, len: 9, node: 4, merged: false },
    { role: Role.Content, len: 900, node: 5, merged: false },
  ]);
});

it("draws an archive of 100,000 entries with a bounded number of parts, every byte counted", () => {
  const slices: Slice[] = [];
  let at = 0;
  for (let i = 0; i < 100_000; i++) {
    slices.push(slice(at, 60, Role.Structure, i * 2), slice(at + 60, 120, Role.Content, i * 2 + 1));
    at += 180;
  }
  slices.push(slice(at, 9_000_000, Role.Structure, -1));
  const parts = barParts(slices);
  expect(parts.length).toBeLessThanOrEqual(MAX_BAR_PARTS + 1);
  expect(parts.every((p) => p.merged)).toBe(true);
  // Every byte keeps its colour: the entries' content is a third of the file, as the list under the bar says.
  expect(parts.reduce((n, p) => n + p.len, 0)).toBe(at + 9_000_000);
  expect(parts.filter((p) => p.role === Role.Content).reduce((n, p) => n + p.len, 0)).toBe(12_000_000);
  // Headers and content mix along the entries; the directory at the end is structure alone.
  expect(new Set(parts.slice(0, 10).map((p) => p.role))).toEqual(new Set([Role.Structure, Role.Content]));
  expect(parts.at(-1)!.role).toBe(Role.Structure);
  expect(parts.at(-1)!.len).toBeGreaterThan(8_900_000);
  // Neighbours of one colour are one part.
  expect(parts.every((p, i) => i === 0 || p.role !== parts[i - 1].role)).toBe(true);
});

it("points a merged part at its largest slice of that colour that leads somewhere", () => {
  const slices: Slice[] = [];
  for (let i = 0; i < MAX_BAR_PARTS * 4; i++) slices.push(slice(i * 10, i === 5 ? 30 : i === 6 ? 50 : 10, Role.Content, i === 5 ? 500 : -1));
  slices.push(slice(MAX_BAR_PARTS * 40, MAX_BAR_PARTS * 400, Role.Hidden, 9_999));
  expect(barParts(slices)).toEqual([
    { role: Role.Content, len: MAX_BAR_PARTS * 40 + 60, node: 500, merged: true },
    { role: Role.Hidden, len: MAX_BAR_PARTS * 400, node: 9_999, merged: false },
  ]);
});
