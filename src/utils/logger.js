// Thin variadic wrapper around @tauri-apps/plugin-log.
// Lets the rest of the codebase keep console.*-style call sites
// (multi-arg, mixed string/object) while routing through the Rust logger.
// Falls back to console.* when running outside Tauri (dev server, tests).
import { debug, info, warn, error } from "@tauri-apps/plugin-log";

/** @param {unknown[]} args @returns {string} */
function format(args) {
  return args
    .map((/** @type {unknown} */ a) => {
      if (typeof a === "string") {
        return a;
      }
      if (a instanceof Error) {
        return `${a.message}\n${a.stack ?? ""}`;
      }
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

const isTauri = () => typeof window !== "undefined" && !!window.__TAURI__;

/**
 * @param {(msg: string) => Promise<void>} fn
 * @param {(...args: unknown[]) => void} consoleFn
 */
const send =
  (fn, consoleFn) =>
  (/** @type {unknown[]} */ ...args) => {
    if (isTauri()) {
      fn(format(args)).catch(() => {
        /* never let the logger throw into the app */
      });
    } else {
      consoleFn(...args);
    }
  };

/* eslint-disable no-console */
export const logger = {
  debug: send(debug, console.debug),
  info: send(info, console.info),
  warn: send(warn, console.warn),
  error: send(error, console.error),
};
/* eslint-enable no-console */
