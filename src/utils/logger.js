// Thin variadic wrapper around @tauri-apps/plugin-log.
// Lets the rest of the codebase keep console.*-style call sites
// (multi-arg, mixed string/object) while routing through the Rust logger.
import { debug, info, warn, error } from "@tauri-apps/plugin-log";

function format(args) {
  return args
    .map((a) => {
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

const send =
  (fn) =>
  (...args) => {
    fn(format(args)).catch(() => {
      /* never let the logger throw into the app */
    });
  };

export const logger = {
  debug: send(debug),
  info: send(info),
  warn: send(warn),
  error: send(error),
};
