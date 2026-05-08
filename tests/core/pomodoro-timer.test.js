import { PomodoroTimer } from "../../src/core/pomodoro-timer.js";

// Minimal HTML structure that satisfies every DOM access in PomodoroTimer.init().
const TIMER_DOM = `
  <div class="container">
    <div class="timer-container">
      <span id="timer-minutes"></span>
      <span id="timer-seconds"></span>
      <span id="status-text"></span>
      <button id="play-pause-btn">
        <span id="play-icon"></span>
        <span id="pause-icon"></span>
      </button>
      <button id="stop-btn">
        <span id="stop-icon"></span>
        <span id="undo-icon"></span>
      </button>
      <button id="skip-btn">
        <span id="skip-coffee-icon"></span>
        <span id="skip-sleep-icon"></span>
        <span id="skip-brain-icon"></span>
        <span id="skip-default-icon"></span>
      </button>
      <div id="progress-dots"></div>
    </div>
  </div>
`;

describe("PomodoroTimer", () => {
  let timer;

  beforeEach(() => {
    document.body.innerHTML = TIMER_DOM;
    timer = new PomodoroTimer();
  });

  afterEach(() => {
    // Clear the midnight-monitoring interval so Vitest doesn't hang.
    timer.stopMidnightMonitoring();
  });

  describe("initial state", () => {
    it("starts in focus mode", () => {
      expect(timer.currentMode).toBe("focus");
    });

    it("starts with 25 minutes remaining", () => {
      expect(timer.timeRemaining).toBe(25 * 60);
    });

    it("starts not running", () => {
      expect(timer.isRunning).toBe(false);
    });

    it("starts not paused", () => {
      expect(timer.isPaused).toBe(false);
    });

    it("starts with 0 completed pomodoros", () => {
      expect(timer.completedPomodoros).toBe(0);
    });
  });

  describe("adjustTimer", () => {
    it("adds minutes (converted to seconds)", () => {
      timer.adjustTimer(5);
      expect(timer.timeRemaining).toBe(25 * 60 + 300);
    });

    it("clamps at 0 for a large negative adjustment", () => {
      timer.adjustTimer(-1000);
      expect(timer.timeRemaining).toBe(0);
    });
  });

  describe("resetTimer", () => {
    it("restores focus duration and clears running state", () => {
      timer.timeRemaining = 60;
      timer.isRunning = true;
      timer.sessionStartTime = new Date();

      timer.resetTimer();

      expect(timer.timeRemaining).toBe(timer.durations.focus);
      expect(timer.isRunning).toBe(false);
      expect(timer.sessionStartTime).toBeNull();
    });
  });
});
