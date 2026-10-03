export function formatSize(bytes: number, isDir: boolean, isParent: boolean): string {
  if (isParent || isDir) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
export function formatEpoch(sec: number, isParent: boolean): string {
  if (isParent || !sec) return '';
  const d = new Date(sec * 1000);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}
export function formatSpeed(bps: number): string {
  if (bps >= 1024 * 1024) return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
  if (bps >= 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
  return `${bps.toFixed(0)} B/s`;
}
export function passwordStrength(p: string): 0 | 1 | 2 {
  if (!p || p.length < 8) return 0;
  let score = 0;
  if (p.length >= 12) score++;
  if (/[a-z]/.test(p) && /[A-Z]/.test(p)) score++;
  if (/\d/.test(p)) score++;
  if (/[^A-Za-z0-9]/.test(p)) score++;
  return score >= 3 ? 2 : score >= 1 ? 1 : 0;
}
/** Tìm nhanh bằng bàn phím (giống WPF TextSearch / v1 SftpTypeAheadTests). */
export class TypeAhead {
  private prefix = '';
  private timer: ReturnType<typeof setTimeout> | null = null;
  constructor(private resetMs = 1500) {}
  /** Trả về chỉ số mục cần chọn, hoặc -1. */
  next(char: string, names: string[], currentIndex: number): number {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => (this.prefix = ''), this.resetMs);
    const c = char.toLowerCase();
    const lower = names.map((n) => n.toLowerCase());
    // Lặp cùng một chữ → nhảy tới mục kế tiếp có chữ đầu đó.
    if (this.prefix.length >= 1 && this.prefix === c.repeat(this.prefix.length)) {
      const start = currentIndex + 1;
      for (let k = 0; k < lower.length; k++) {
        const i = (start + k) % lower.length;
        if (lower[i].startsWith(c)) { this.prefix = c; return i; }
      }
    }
    const candidate = this.prefix + c;
    const found = lower.findIndex((n) => n.startsWith(candidate));
    if (found >= 0) { this.prefix = candidate; return found; }
    // Không khớp: giữ nguyên chọn, nhưng nếu prefix mới chỉ 1 ký tự thì thử ký tự đó
    if (this.prefix.length > 0) {
      const single = lower.findIndex((n) => n.startsWith(c));
      if (single >= 0) { this.prefix = c; return single; }
    }
    return -1;
  }
}
