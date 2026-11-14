// The zoom bucket of a zoom factor (SI-137): the exponent of the smallest power of two at or above it, within ±8. The
// same rule as Rust's `tt_render::canvas::bucket_of` (checked against it in `bucket.test.ts`).
export function bucketOf(zoom: number): number {
  if (!(Number.isFinite(zoom) && zoom > 0)) return 0;
  let b = -8;
  let top = 1 / 256;
  while (top < zoom && b < 8) {
    b++;
    top *= 2;
  }
  return b;
}
