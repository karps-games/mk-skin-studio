const ua = typeof navigator === "undefined" ? "" : `${navigator.userAgent} ${navigator.platform ?? ""}`;

export const IS_MAC = /Macintosh|Mac OS X|MacIntel/i.test(ua) && !/Windows/i.test(ua);
export const IS_WINDOWS = /Windows/i.test(ua);
export const IS_LINUX = !IS_MAC && !IS_WINDOWS && /Linux|X11|CrOS|BSD/i.test(ua);
export const OS: "mac" | "windows" | "linux" | "other" = IS_MAC ? "mac" : IS_WINDOWS ? "windows" : IS_LINUX ? "linux" : "other";

export function keys(text: string): string {
  return IS_MAC ? text.replace(/\bCtrl\+/g, "⌘") : text;
}
