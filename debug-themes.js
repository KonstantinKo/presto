// Debug script for theme verification

console.log("=== TEMA DEBUG INFO ===");

const html = document.documentElement;
console.log("data-theme:", html.getAttribute("data-theme"));
console.log("data-timer-theme:", html.getAttribute("data-timer-theme"));

const computedStyle = getComputedStyle(html);
console.log("CSS Variables:");
console.log("--focus-color:", computedStyle.getPropertyValue("--focus-color").trim());
console.log("--focus-bg:", computedStyle.getPropertyValue("--focus-bg").trim());
console.log("--focus-timer-color:", computedStyle.getPropertyValue("--focus-timer-color").trim());
console.log("--matrix-text:", computedStyle.getPropertyValue("--matrix-text").trim());

console.log("localStorage theme-preference:", localStorage.getItem("theme-preference"));
console.log("localStorage timer-theme-preference:", localStorage.getItem("timer-theme-preference"));

console.log("System prefers dark:", window.matchMedia("(prefers-color-scheme: dark)").matches);

const testElement = document.createElement("div");
testElement.style.cssText = "color: var(--matrix-text, red);";
document.body.appendChild(testElement);
const matrixColor = getComputedStyle(testElement).color;
document.body.removeChild(testElement);
console.log("Matrix text color test:", matrixColor);

console.log("Stylesheets loaded:");
Array.from(document.styleSheets).forEach((sheet, i) => {
  try {
    console.log(`${i}: ${sheet.href || "inline"}`);
  } catch (e) {
    console.log(`${i}: (CORS protected)`);
  }
});

console.log("=== END DEBUG ===");
