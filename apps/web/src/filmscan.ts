export type FilmScanEdge = "top" | "right" | "bottom" | "left";

export interface FilmScanReport {
  availability: "inspected" | "unavailable";
  perforationEdges: FilmScanEdge[];
  perforationRepeats: number;
  frameEdges: FilmScanEdge[];
  evidenceStrength: "none" | "limited" | "corroborated";
  /** Optional suggested frame [left, top, right, bottom], in raster fractions. */
  frameBounds?: [number, number, number, number];
}

const EDGES: readonly FilmScanEdge[] = ["top", "right", "bottom", "left"];
const MAX_PIXELS = 6_000_000;
const MIN_SIDE = 96;

function unavailable(): FilmScanReport {
  return {
    availability: "unavailable",
    perforationEdges: [],
    perforationRepeats: 0,
    frameEdges: [],
    evidenceStrength: "none",
  };
}

function median(values: readonly number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >>> 1;
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function sample(
  pixels: Uint8Array,
  width: number,
  height: number,
  edge: FilmScanEdge,
  along: number,
  depth: number,
): number {
  switch (edge) {
    case "top":
      return pixels[depth * width + along];
    case "right":
      return pixels[along * width + width - 1 - depth];
    case "bottom":
      return pixels[(height - 1 - depth) * width + along];
    case "left":
      return pixels[along * width + depth];
  }
}

function edgeProfile(
  pixels: Uint8Array,
  width: number,
  height: number,
  edge: FilmScanEdge,
  near: number,
  far: number,
): number[] {
  const alongLength = edge === "top" || edge === "bottom" ? width : height;
  const acrossLength = edge === "top" || edge === "bottom" ? height : width;
  const start = Math.max(1, Math.round(acrossLength * near));
  const end = Math.min(acrossLength - 1, Math.max(start + 1, Math.round(acrossLength * far)));
  const profile = new Array<number>(alongLength);
  for (let along = 0; along < alongLength; along++) {
    let sum = 0;
    for (let depth = start; depth < end; depth++) sum += sample(pixels, width, height, edge, along, depth);
    profile[along] = sum / (end - start);
  }
  return profile;
}

/** Find evenly spaced, high-contrast runs rather than isolated texture or grain. */
function repeatedOpenings(profile: readonly number[]): number | null {
  const center = median(profile);
  const deviations = profile.map((value) => Math.abs(value - center));
  const threshold = Math.max(34, median(deviations) * 3.2);
  const minRun = Math.max(2, Math.round(profile.length * 0.004));
  const maxRun = Math.round(profile.length * 0.08);
  const centers: number[] = [];
  let start = -1;
  for (let i = 0; i <= profile.length; i++) {
    const active = i < profile.length && deviations[i] >= threshold;
    if (active && start < 0) start = i;
    if (!active && start >= 0) {
      const length = i - start;
      if (length >= minRun && length <= maxRun) centers.push((start + i - 1) / 2);
      start = -1;
    }
  }
  if (centers.length < 4) return null;

  const gaps = centers.slice(1).map((point, i) => point - centers[i]);
  const mean = gaps.reduce((sum, gap) => sum + gap, 0) / gaps.length;
  if (mean < profile.length * 0.018 || mean > profile.length * 0.24) return null;
  const variance = gaps.reduce((sum, gap) => sum + (gap - mean) ** 2, 0) / gaps.length;
  const coefficientOfVariation = Math.sqrt(variance) / mean;
  return coefficientOfVariation <= 0.22 ? centers.length : null;
}

/** Check for a strong transition at a consistent depth along most of an edge. */
function frameBoundary(
  pixels: Uint8Array,
  width: number,
  height: number,
  edge: FilmScanEdge,
): number | null {
  const alongLength = edge === "top" || edge === "bottom" ? width : height;
  const acrossLength = edge === "top" || edge === "bottom" ? height : width;
  const sampleCount = 48;
  const alongBand = Math.max(1, Math.floor(alongLength / 256));
  const window = Math.max(2, Math.round(acrossLength * 0.01));
  const firstDepth = Math.max(window * 2, Math.round(acrossLength * 0.02));
  const lastDepth = Math.min(acrossLength - window * 2, Math.round(acrossLength * 0.25));
  const depthStep = Math.max(1, Math.round(acrossLength * 0.005));
  const candidates: number[] = [];

  for (let i = 0; i < sampleCount; i++) {
    const along = Math.round(alongLength * (0.08 + 0.84 * (i + 0.5) / sampleCount));
    let bestContrast = 0;
    let bestDepth = -1;
    for (let depth = firstDepth; depth <= lastDepth; depth += depthStep) {
      let outside = 0;
      let inside = 0;
      let count = 0;
      for (let a = Math.max(0, along - alongBand); a <= Math.min(alongLength - 1, along + alongBand); a++) {
        for (let d = depth - window; d < depth; d++) {
          outside += sample(pixels, width, height, edge, a, d);
          count++;
        }
        for (let d = depth; d < depth + window; d++) inside += sample(pixels, width, height, edge, a, d);
      }
      const contrast = Math.abs(inside / count - outside / count);
      if (contrast > bestContrast) {
        bestContrast = contrast;
        bestDepth = depth;
      }
    }
    if (bestContrast >= 32) candidates.push(bestDepth);
  }

  if (candidates.length < sampleCount * 0.55) return null;
  const tolerance = Math.max(2, Math.round(acrossLength * 0.015));
  const clusters = candidates.map((depth) => candidates.filter((candidate) => Math.abs(candidate - depth) <= tolerance));
  const best = clusters.reduce((a, b) => a.length >= b.length ? a : b);
  return best.length >= sampleCount * 0.55 ? median(best) / acrossLength : null;
}

/**
 * Detects visual scan clues in a bounded grayscale raster. Results describe
 * repeated image patterns only; they do not establish that a photo came from film.
 */
export function inspectFilmScan(pixels: Uint8Array, width: number, height: number): FilmScanReport {
  const size = width * height;
  if (
    !Number.isSafeInteger(width)
    || !Number.isSafeInteger(height)
    || width < MIN_SIDE
    || height < MIN_SIDE
    || size > MAX_PIXELS
    || pixels.length !== size
  ) return unavailable();

  const perforationByEdge = new Map<FilmScanEdge, number>();
  for (const edge of EDGES) {
    const repeated = [
      edgeProfile(pixels, width, height, edge, 0.035, 0.11),
      edgeProfile(pixels, width, height, edge, 0.08, 0.17),
      edgeProfile(pixels, width, height, edge, 0.14, 0.24),
    ].map(repeatedOpenings).filter((count): count is number => count !== null);
    if (repeated.length > 0) perforationByEdge.set(edge, Math.max(...repeated));
  }

  const perforationEdges = EDGES.filter((edge) => perforationByEdge.has(edge));
  const frames = new Map(EDGES.map((edge) => [edge, frameBoundary(pixels, width, height, edge)]));
  const frameEdges = EDGES.filter((edge) => frames.get(edge) !== null);
  const frameBounds: [number, number, number, number] | undefined = frameEdges.length === 4
    ? [frames.get("left")!, frames.get("top")!, 1 - frames.get("right")!, 1 - frames.get("bottom")!]
    : undefined;
  const evidenceStrength = perforationEdges.length > 0 && frameEdges.length > 0
    ? "corroborated"
    : perforationEdges.length > 0 || frameEdges.length > 0
      ? "limited"
      : "none";
  return {
    availability: "inspected",
    perforationEdges,
    perforationRepeats: Math.max(0, ...perforationByEdge.values()),
    frameEdges,
    evidenceStrength,
    ...(frameBounds ? { frameBounds } : {}),
  };
}
