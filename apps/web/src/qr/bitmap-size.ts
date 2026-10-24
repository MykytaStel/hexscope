/**
 * Decode a large photo at QR scan size when its dimensions are already known.
 * Only one resize axis is set: the browser keeps the source aspect ratio, and
 * the longest axis remains capped even when EXIF rotates the image.
 */
export function bitmapResizeOptions(
  dimensions: readonly [number, number] | null | undefined,
  maxDimension = 2400,
): ImageBitmapOptions | undefined {
  const [width, height] = dimensions ?? [];
  if (
    typeof width !== "number" || typeof height !== "number" ||
    !Number.isFinite(width) || !Number.isFinite(height) ||
    width <= 0 || height <= 0 || !Number.isInteger(maxDimension) || maxDimension <= 0
  ) return undefined;
  if (Math.max(width, height) <= maxDimension) return undefined;
  return width >= height
    ? { resizeWidth: maxDimension, resizeQuality: "high" }
    : { resizeHeight: maxDimension, resizeQuality: "high" };
}
