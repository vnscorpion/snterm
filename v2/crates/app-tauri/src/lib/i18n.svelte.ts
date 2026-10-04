import vi from './i18n/vi.json';
import en from './i18n/en.json';

type Dict = Record<string, string>;
const dicts: Record<string, Dict> = { vi: vi as Dict, en: en as Dict };

let current = $state<'vi' | 'en'>('vi');

export function setLanguage(lang: string) {
  current = lang?.toLowerCase() === 'vi' ? 'vi' : 'en';
  document.documentElement.lang = current;
}
export function getLanguage() { return current; }

/** t('Str_ConfirmDeletePrompt', name) — thay {0},{1}… như string.Format của v1. */
export function t(key: string, ...args: (string | number)[]): string {
  const d = dicts[current] ?? dicts.en;
  let s = d[key] ?? dicts.en[key] ?? key;
  args.forEach((a, i) => { s = s.split(`{${i}}`).join(String(a)); });
  return s;
}
