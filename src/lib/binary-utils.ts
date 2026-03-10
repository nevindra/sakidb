/** Detected binary format from magic bytes. */
export type BinaryFormat =
  | { kind: 'image'; mime: string }
  | { kind: 'pdf' }
  | { kind: 'archive'; format: 'zip' | 'gzip' | 'bz2' | 'xz' | '7z' }
  | { kind: 'unknown' };

/** Detect binary format from magic bytes. */
export function detectBinaryFormat(bytes: number[]): BinaryFormat {
  if (bytes.length < 4) return { kind: 'unknown' };

  // Images
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47)
    return { kind: 'image', mime: 'image/png' };
  if (bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff)
    return { kind: 'image', mime: 'image/jpeg' };
  if (bytes[0] === 0x47 && bytes[1] === 0x49 && bytes[2] === 0x46)
    return { kind: 'image', mime: 'image/gif' };
  if (
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46 &&
    bytes.length > 11 &&
    bytes[8] === 0x57 && bytes[9] === 0x45 && bytes[10] === 0x42 && bytes[11] === 0x50
  )
    return { kind: 'image', mime: 'image/webp' };
  if (bytes[0] === 0x42 && bytes[1] === 0x4d)
    return { kind: 'image', mime: 'image/bmp' };
  if (
    bytes.length > 11 &&
    bytes[4] === 0x66 && bytes[5] === 0x74 && bytes[6] === 0x79 && bytes[7] === 0x70 &&
    bytes[8] === 0x61 && bytes[9] === 0x76 && bytes[10] === 0x69 && bytes[11] === 0x66
  )
    return { kind: 'image', mime: 'image/avif' };

  // PDF
  if (bytes[0] === 0x25 && bytes[1] === 0x50 && bytes[2] === 0x44 && bytes[3] === 0x46)
    return { kind: 'pdf' };

  // Archives
  if (bytes[0] === 0x50 && bytes[1] === 0x4b && bytes[2] === 0x03 && bytes[3] === 0x04)
    return { kind: 'archive', format: 'zip' };
  if (bytes[0] === 0x1f && bytes[1] === 0x8b)
    return { kind: 'archive', format: 'gzip' };
  if (bytes[0] === 0x42 && bytes[1] === 0x5a && bytes[2] === 0x68)
    return { kind: 'archive', format: 'bz2' };
  if (bytes[0] === 0xfd && bytes[1] === 0x37 && bytes[2] === 0x7a && bytes[3] === 0x58)
    return { kind: 'archive', format: 'xz' };
  if (
    bytes.length > 5 &&
    bytes[0] === 0x37 && bytes[1] === 0x7a && bytes[2] === 0xbc && bytes[3] === 0xaf &&
    bytes[4] === 0x27 && bytes[5] === 0x1c
  )
    return { kind: 'archive', format: '7z' };

  return { kind: 'unknown' };
}

/** Human-readable file size. */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** Short label for the format, e.g. "PNG", "PDF", "ZIP". */
export function formatLabel(format: BinaryFormat): string {
  switch (format.kind) {
    case 'image': {
      const sub = format.mime.split('/')[1]?.toUpperCase() ?? 'IMG';
      return sub === 'JPEG' ? 'JPG' : sub;
    }
    case 'pdf': return 'PDF';
    case 'archive': return format.format.toUpperCase();
    case 'unknown': return 'BIN';
  }
}

/** Inline label with size, e.g. "PNG · 120 KB". */
export function formatBinaryLabel(format: BinaryFormat, sizeBytes: number): string {
  return `${formatLabel(format)} · ${formatFileSize(sizeBytes)}`;
}

/** Get MIME type for creating object URLs. */
export function getMimeType(format: BinaryFormat): string {
  switch (format.kind) {
    case 'image': return format.mime;
    case 'pdf': return 'application/pdf';
    case 'archive':
      switch (format.format) {
        case 'zip': return 'application/zip';
        case 'gzip': return 'application/gzip';
        case 'bz2': return 'application/x-bzip2';
        case 'xz': return 'application/x-xz';
        case '7z': return 'application/x-7z-compressed';
      }
      break;
    case 'unknown': return 'application/octet-stream';
  }
  return 'application/octet-stream';
}

/** Get a suggested file extension (without dot). */
export function getExtension(format: BinaryFormat): string {
  switch (format.kind) {
    case 'image': {
      const sub = format.mime.split('/')[1] ?? 'bin';
      return sub === 'jpeg' ? 'jpg' : sub;
    }
    case 'pdf': return 'pdf';
    case 'archive': return format.format === 'gzip' ? 'gz' : format.format;
    case 'unknown': return 'bin';
  }
}

/** Create an object URL from bytes. Caller must revoke. */
export function bytesToObjectUrl(bytes: number[], mime: string): string {
  return URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: mime }));
}
