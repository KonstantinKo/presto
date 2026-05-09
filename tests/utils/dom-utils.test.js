import { DOMUtils } from "../../src/utils/common-utils.js";

describe("DOMUtils.toggleClass", () => {
  let el;

  beforeEach(() => {
    el = document.createElement("div");
    document.body.appendChild(el);
  });

  afterEach(() => {
    el.remove();
  });

  it("adds a class when condition is true", () => {
    DOMUtils.toggleClass(el, "active", true);
    expect(el.classList.contains("active")).toBe(true);
  });

  it("removes a class when condition is false", () => {
    el.classList.add("active");
    DOMUtils.toggleClass(el, "active", false);
    expect(el.classList.contains("active")).toBe(false);
  });

  it("does not add a class that is already present when condition is true", () => {
    el.classList.add("active");
    DOMUtils.toggleClass(el, "active", true);
    expect(el.classList.contains("active")).toBe(true);
  });

  it("toggles the class on when condition is null and class is absent", () => {
    DOMUtils.toggleClass(el, "active", null);
    expect(el.classList.contains("active")).toBe(true);
  });

  it("toggles the class off when condition is null and class is present", () => {
    el.classList.add("active");
    DOMUtils.toggleClass(el, "active", null);
    expect(el.classList.contains("active")).toBe(false);
  });

  it("does nothing when element is null", () => {
    expect(() => DOMUtils.toggleClass(null, "active")).not.toThrow();
  });
});

describe("DOMUtils.updateElementText", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("sets textContent on the matching element", () => {
    const el = document.createElement("span");
    el.id = "my-span";
    document.body.appendChild(el);
    DOMUtils.updateElementText("my-span", "Hello!");
    expect(el.textContent).toBe("Hello!");
  });

  it("replaces existing text content", () => {
    const el = document.createElement("span");
    el.id = "target";
    el.textContent = "old";
    document.body.appendChild(el);
    DOMUtils.updateElementText("target", "new");
    expect(el.textContent).toBe("new");
  });

  it("does nothing when element ID does not exist", () => {
    expect(() => DOMUtils.updateElementText("nonexistent", "text")).not.toThrow();
  });
});

describe("DOMUtils.createModal", () => {
  afterEach(() => {
    document.querySelectorAll(".modal-overlay").forEach((m) => m.remove());
  });

  it("appends a modal overlay to the body", () => {
    DOMUtils.createModal("My Title", "<p>Body</p>");
    expect(document.querySelector(".modal-overlay")).not.toBeNull();
  });

  it("sets the modal title", () => {
    DOMUtils.createModal("My Title", "<p>Body</p>");
    expect(document.querySelector(".modal-overlay h3").textContent).toBe("My Title");
  });

  it("renders the provided HTML content in the modal body", () => {
    DOMUtils.createModal("Title", "<p id='test-body'>Content</p>");
    expect(document.querySelector(".modal-body #test-body")).not.toBeNull();
  });

  it("replaces an existing modal rather than stacking", () => {
    DOMUtils.createModal("First", "");
    DOMUtils.createModal("Second", "");
    const overlays = document.querySelectorAll(".modal-overlay");
    expect(overlays.length).toBe(1);
    expect(document.querySelector(".modal-overlay h3").textContent).toBe("Second");
  });

  it("appends an extra className to the overlay", () => {
    DOMUtils.createModal("Title", "", "wide-modal");
    expect(document.querySelector(".modal-overlay").classList.contains("wide-modal")).toBe(true);
  });

  it("removes the show class when close button is clicked", () => {
    const overlay = DOMUtils.createModal("Title", "");
    overlay.classList.add("show");
    document.querySelector(".close-btn").click();
    expect(overlay.classList.contains("show")).toBe(false);
  });

  it("closes when clicking directly on the overlay background", () => {
    const overlay = DOMUtils.createModal("Title", "");
    overlay.classList.add("show");
    overlay.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(overlay.classList.contains("show")).toBe(false);
  });

  it("returns the overlay element", () => {
    const overlay = DOMUtils.createModal("Title", "");
    expect(overlay.classList.contains("modal-overlay")).toBe(true);
  });
});

describe("DOMUtils.closeModal", () => {
  afterEach(() => {
    document.querySelectorAll(".modal-overlay").forEach((m) => m.remove());
  });

  it("does nothing when no modal overlay exists", () => {
    expect(() => DOMUtils.closeModal()).not.toThrow();
  });

  it("removes the show class from a passed overlay element", () => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay show";
    document.body.appendChild(overlay);
    DOMUtils.closeModal(overlay);
    expect(overlay.classList.contains("show")).toBe(false);
  });

  it("finds and closes the modal overlay when no element is passed", () => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay show";
    document.body.appendChild(overlay);
    DOMUtils.closeModal();
    expect(overlay.classList.contains("show")).toBe(false);
  });
});
