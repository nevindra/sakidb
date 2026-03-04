import type { CellValue } from '$lib/types';

/** Detect MIME type from magic bytes, returns null if not a recognized image. */
export function detectImageMime(bytes: number[]): string | null {
  if (bytes.length < 4) return null;
  // PNG
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47)
    return 'image/png';
  // JPEG
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) return 'image/jpeg';
  // GIF
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46) return 'image/gif';
  // WebP (RIFF....WEBP)
  if (
    bytes[0] === 0x52 &&
    bytes[1] === 0x49 &&
    bytes[2] === 0x46 &&
    bytes[3] === 0x46 &&
    bytes.length > 11 &&
    bytes[8] === 0x57 &&
    bytes[9] === 0x45 &&
    bytes[10] === 0x42 &&
    bytes[11] === 0x50
  )
    return 'image/webp';
  // BMP
  if (bytes[0] === 0x42 && bytes[1] === 0x4d) return 'image/bmp';
  // AVIF (....ftypavif)
  if (
    bytes.length > 11 &&
    bytes[4] === 0x66 &&
    bytes[5] === 0x74 &&
    bytes[6] === 0x79 &&
    bytes[7] === 0x70
  )
    return 'image/avif';
  return null;
}

/** Convert a byte array to an object URL. Caller must revoke with URL.revokeObjectURL(). */
export function bytesToObjectUrl(bytes: number[], mime: string): string {
  return URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: mime }));
}

const IMAGE_URL_RE = /\.(png|jpe?g|gif|webp|bmp|svg|ico|avif)(\?.*)?$/i;

/** Check if a text string looks like an image URL or data URI. */
export function isImageUrl(text: string): boolean {
  if (text.startsWith('data:image/')) return true;
  try {
    const url = new URL(text);
    return (
      (url.protocol === 'http:' || url.protocol === 'https:') && IMAGE_URL_RE.test(url.pathname)
    );
  } catch {
    return false;
  }
}

/** Extract an image src from a CellValue, or null if not an image. */
export function cellToImageSrc(cell: CellValue): string | null {
  if (cell === 'Null') return null;
  if ('Bytes' in cell) {
    const mime = detectImageMime(cell.Bytes);
    if (!mime) return null;
    return bytesToObjectUrl(cell.Bytes, mime);
  }
  if ('Text' in cell && isImageUrl(cell.Text)) {
    return cell.Text;
  }
  return null;
}
