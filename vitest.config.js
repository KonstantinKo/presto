import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
<<<<<<< HEAD
    environment: "happy-dom",
    globals: true,
    setupFiles: ["./tests/setup.js"],
    include: ["tests/**/*.test.js"],
=======
    environment: "jsdom",
    setupFiles: ["./vitest.setup.js"],
>>>>>>> 2dca63af77569f649348bfd32f37fc1f4f860dab
  },
});
