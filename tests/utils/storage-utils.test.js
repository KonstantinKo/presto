import { StorageUtils } from "../../src/utils/common-utils.js";

describe("StorageUtils.saveToLocalStorage", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("saves an object and returns true", () => {
    const result = StorageUtils.saveToLocalStorage("test-key", { a: 1 });
    expect(result).toBe(true);
    expect(localStorage.getItem("test-key")).toBe(JSON.stringify({ a: 1 }));
  });

  it("saves an array value", () => {
    StorageUtils.saveToLocalStorage("arr-key", [1, 2, 3]);
    expect(localStorage.getItem("arr-key")).toBe("[1,2,3]");
  });

  it("saves a string value", () => {
    StorageUtils.saveToLocalStorage("str-key", "hello");
    expect(localStorage.getItem("str-key")).toBe('"hello"');
  });

  it("saves a number value", () => {
    StorageUtils.saveToLocalStorage("num-key", 42);
    expect(localStorage.getItem("num-key")).toBe("42");
  });

  it("overwrites an existing key", () => {
    StorageUtils.saveToLocalStorage("key", { v: 1 });
    StorageUtils.saveToLocalStorage("key", { v: 2 });
    expect(JSON.parse(localStorage.getItem("key"))).toEqual({ v: 2 });
  });
});

describe("StorageUtils.loadFromLocalStorage", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns null when the key does not exist", () => {
    expect(StorageUtils.loadFromLocalStorage("missing")).toBeNull();
  });

  it("returns the provided defaultValue when the key does not exist", () => {
    expect(StorageUtils.loadFromLocalStorage("missing", [])).toEqual([]);
  });

  it("loads and parses a previously saved object", () => {
    localStorage.setItem("obj-key", JSON.stringify({ x: 42 }));
    expect(StorageUtils.loadFromLocalStorage("obj-key")).toEqual({ x: 42 });
  });

  it("loads and parses a previously saved array", () => {
    localStorage.setItem("arr-key", JSON.stringify([1, 2, 3]));
    expect(StorageUtils.loadFromLocalStorage("arr-key")).toEqual([1, 2, 3]);
  });

  it("returns defaultValue when stored data is malformed JSON", () => {
    localStorage.setItem("bad-key", "{bad json");
    expect(StorageUtils.loadFromLocalStorage("bad-key", "fallback")).toBe("fallback");
  });

  it("round-trips a complex object correctly", () => {
    const data = { name: "test", values: [1, 2], nested: { flag: true } };
    StorageUtils.saveToLocalStorage("round-trip", data);
    expect(StorageUtils.loadFromLocalStorage("round-trip")).toEqual(data);
  });
});
