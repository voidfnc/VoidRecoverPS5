// Wording the plan forbids in any language: a checksum or a source listing never proves a file is
// authentic, safe or compatible. Every shipped language needs its own list here (checked by tests).
// This file is skipped by the "UI source" scan because its patterns are the words themselves.

export const BANNED: Record<string, RegExp[]> = {
  en: [/verified (safe|firmware|authentic)/i, /\bauthentic\b/i, /100% safe/i, /jailbreak[- ]compatible/i, /guaranteed/i],
  ru: [
    /подлинн/i,
    /аутентичн/i,
    /гарантир/i,
    /100\s?%\s?безопасн/i,
    /совместим\S*\s+с\s+джейлбрейк/i,
    /проверенн\S*\s+(безопасн|прошивк)/i,
  ],
  "zh-CN": [/真实可靠|正品|真品|保证|保障安全|100\s?[%％]\s?安全|绝对安全|完全安全|已验证安全|兼容越狱/],
  es: [/aut[ée]ntic/i, /genuin/i, /garantiz|garant[íi]a/i, /100\s?%\s?segur/i, /compatible con (el )?jailbreak/i, /verificad[oa]s?\s+(como\s+)?segur/i],
  fr: [/authentic|authenticit/i, /garanti|garantie/i, /100\s?%\s?s[ûu]r/i, /compatible (avec )?(le )?jailbreak/i, /v[ée]rifi[ée]s?\s+(comme\s+)?s[ûu]r/i],
  de: [/authentisch|echtheit/i, /garantier|garantie/i, /100\s?%\s?sicher/i, /jailbreak-kompatibel|kompatibel mit (dem )?jailbreak/i, /(verifiziert|überprüft|geprüft)e?[nrs]?\s+sicher/i],
  "pt-BR": [/aut[êe]ntic/i, /genu[íi]n/i, /garant/i, /100\s?%\s?segur/i, /compat[íi]vel com (o )?jailbreak/i, /verificad[oa]s?\s+(como\s+)?segur/i],
  tr: [/orijinal|otantik|özgün/i, /garanti/i, /%\s?100\s?güvenli|100\s?%\s?güvenli/i, /jailbreak ile uyumlu|jailbreak uyumlu/i, /doğrulanmış\s+güvenli/i],
  pl: [/autentyczn/i, /gwarantu|gwarancj/i, /100\s?%\s?bezpiecz/i, /zgodn\S*\s+z\s+jailbreak/i, /zweryfikowan\S*\s+bezpiecz/i],
  ja: [/本物|正規品|保証|100\s?[%％]\s?安全|完全に安全|(脱獄|ジェイルブレイク).{0,8}互換|検証済み.{0,6}安全/],
  ko: [/진품|정품|보장|100\s?%\s?안전|완전히 안전|(탈옥|제일브레이크).{0,8}호환|검증된.{0,6}안전/],
  it: [/autentic/i, /genuin/i, /garantit|garanzia/i, /100\s?%\s?sicur/i, /compatibile con (il )?jailbreak/i, /verificat\w*\s+(come\s+)?sicur/i],
};
