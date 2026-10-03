import { describe, expect, it } from 'vitest';
import { TypeAhead, formatSize, passwordStrength } from './format';

const names = ['..', 'alternatives', 'cloud', 'cockpit', 'cron.d', 'cron.daily', ...Array.from({ length: 120 }, (_, i) => `lib${String(i).padStart(3, '0')}`), 'nginx', 'nsswitch.d', 'opt', 'ssh', 'sysconfig'];

describe('TypeAhead (tìm nhanh bằng bàn phím, giống v1 SftpTypeAheadTests)', () => {
  it('chữ đầu chọn mục đầu tiên bắt đầu bằng chữ đó', () => {
    const ta = new TypeAhead();
    expect(names[ta.next('n', names, 0)]).toBe('nginx');
  });
  it('gõ nhiều chữ nhanh thu hẹp theo tiền tố', () => {
    const ta = new TypeAhead();
    let i = ta.next('n', names, 0);
    i = ta.next('s', names, i);
    expect(names[i]).toBe('nsswitch.d');
  });
  it('lặp cùng chữ chuyển sang mục kế tiếp', () => {
    const ta = new TypeAhead();
    let i = ta.next('c', names, 0); expect(names[i]).toBe('cloud');
    i = ta.next('c', names, i); expect(names[i]).toBe('cockpit');
    i = ta.next('c', names, i); expect(names[i]).toBe('cron.d');
  });
  it('không phân biệt hoa thường; không khớp giữ nguyên', () => {
    const ta = new TypeAhead();
    const i = ta.next('N', names, 0); expect(names[i]).toBe('nginx');
    expect(ta.next('z', names, i)).toBe(-1);
  });
  it('dấu chấm chọn dòng ..', () => {
    const ta = new TypeAhead();
    expect(names[ta.next('.', names, 3)]).toBe('..');
  });
  it('reset sau khi nghỉ', async () => {
    const ta = new TypeAhead(50);
    let i = ta.next('n', names, 0); expect(names[i]).toBe('nginx');
    await new Promise((r) => setTimeout(r, 120));
    i = ta.next('s', names, i); expect(names[i]).toBe('ssh');
  });
});

describe('format', () => {
  it('size', () => { expect(formatSize(12288, false, false)).toBe('12.0 KB'); expect(formatSize(5, true, false)).toBe(''); });
  it('strength', () => { expect(passwordStrength('abc')).toBe(0); expect(passwordStrength('Abcdefgh1!xy')).toBe(2); });
});
