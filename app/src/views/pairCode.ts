const SCHEME = "ywza://pair/";

/** What the pairing QR holds. */
export const pairUri = (code: string) => SCHEME + code;

/** The 6 digits from a scanned `ywza://pair/<code>` or bare digits; undefined for anything else. */
export function pairCode(text: string) {
  const t = text.trim();
  const c = (t.startsWith(SCHEME) ? t.slice(SCHEME.length) : t).replace(/\s/g, "");
  return /^\d{6}$/.test(c) ? c : undefined;
}
