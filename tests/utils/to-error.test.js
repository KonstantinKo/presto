import { toError } from "../../src/utils/to-error.js";

describe("toError", () => {
  it("returns the same Error instance when given an Error", () => {
    const err = new Error("original");
    expect(toError(err)).toBe(err);
  });

  it("preserves subclass instances", () => {
    const err = new TypeError("type mismatch");
    expect(toError(err)).toBe(err);
  });

  it("wraps a string in an Error", () => {
    const result = toError("something went wrong");
    expect(result).toBeInstanceOf(Error);
    expect(result.message).toBe("something went wrong");
  });

  it("wraps a number in an Error", () => {
    const result = toError(42);
    expect(result).toBeInstanceOf(Error);
    expect(result.message).toBe("42");
  });

  it("wraps null in an Error with message 'null'", () => {
    const result = toError(null);
    expect(result).toBeInstanceOf(Error);
    expect(result.message).toBe("null");
  });

  it("wraps undefined in an Error with message 'undefined'", () => {
    const result = toError(undefined);
    expect(result).toBeInstanceOf(Error);
    expect(result.message).toBe("undefined");
  });

  it("wraps a plain object using String() conversion", () => {
    const result = toError({ code: 404 });
    expect(result).toBeInstanceOf(Error);
    expect(result.message).toBe("[object Object]");
  });
});
