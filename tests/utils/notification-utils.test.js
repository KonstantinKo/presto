import { NotificationUtils } from "../../src/utils/common-utils.js";

function resetState() {
  document.body.innerHTML = "";
  NotificationUtils.notificationQueue = [];
  NotificationUtils.activeNotifications = new Set();
  NotificationUtils.lastNotificationTimes = new Map();
}

describe("NotificationUtils.showNotificationPing", () => {
  beforeEach(resetState);

  it("creates a notification container if none exists", () => {
    NotificationUtils.showNotificationPing("Hello");
    expect(document.querySelector(".notification-container")).not.toBeNull();
  });

  it("appends a notification element with the message text", () => {
    NotificationUtils.showNotificationPing("Hello World");
    const ping = document.querySelector(".notification-ping");
    expect(ping).not.toBeNull();
    expect(ping.textContent).toBe("Hello World");
  });

  it("assigns the type as a CSS class variant", () => {
    NotificationUtils.showNotificationPing("Good job!", "success");
    expect(document.querySelector(".notification-ping").classList.contains("success")).toBe(true);
  });

  it("uses timerState as the variant class when both type and timerState are provided", () => {
    NotificationUtils.showNotificationPing("Done!", "success", "focus");
    expect(document.querySelector(".notification-ping").classList.contains("focus")).toBe(true);
  });

  it("skips an identical message shown within the cooldown window (spam prevention)", () => {
    NotificationUtils.showNotificationPing("Duplicate");
    NotificationUtils.showNotificationPing("Duplicate");
    const pings = document.querySelectorAll(".notification-ping");
    expect(pings.length).toBe(1);
  });

  it("shows the same message again after the cooldown has expired", () => {
    NotificationUtils.lastNotificationTimes.set("Old msg", Date.now() - 2000);
    NotificationUtils.showNotificationPing("Old msg");
    expect(document.querySelectorAll(".notification-ping").length).toBe(1);
  });

  it("queues a Settings-saved notification when at the simultaneous limit", () => {
    for (let i = 0; i < NotificationUtils.maxSimultaneousNotifications; i++) {
      NotificationUtils.showNotificationPing(`Unique message ${i}`);
    }
    NotificationUtils.showNotificationPing("Settings saved now", "success");
    expect(NotificationUtils.notificationQueue.length).toBe(1);
    expect(NotificationUtils.notificationQueue[0].message).toBe("Settings saved now");
  });

  it("uses 'info' as the variant when no type or timerState is given", () => {
    NotificationUtils.showNotificationPing("Neutral message");
    const ping = document.querySelector(".notification-ping");
    expect(ping.classList.contains("info")).toBe(true);
  });

  it("tracks the new notification ID in activeNotifications", () => {
    NotificationUtils.showNotificationPing("Track me");
    expect(NotificationUtils.activeNotifications.size).toBe(1);
  });
});

describe("NotificationUtils.queueNotification", () => {
  beforeEach(resetState);

  it("adds a notification to the queue when slots are full", () => {
    for (let i = 0; i < NotificationUtils.maxSimultaneousNotifications; i++) {
      NotificationUtils.activeNotifications.add(`fake-id-${i}`);
    }
    NotificationUtils.queueNotification("Queued msg", "info", null);
    expect(NotificationUtils.notificationQueue).toHaveLength(1);
    expect(NotificationUtils.notificationQueue[0].message).toBe("Queued msg");
  });

  it("does not queue the same message twice (no duplicates)", () => {
    for (let i = 0; i < NotificationUtils.maxSimultaneousNotifications; i++) {
      NotificationUtils.activeNotifications.add(`fake-id-${i}`);
    }
    NotificationUtils.queueNotification("Same msg", "info", null);
    NotificationUtils.queueNotification("Same msg", "info", null);
    expect(NotificationUtils.notificationQueue).toHaveLength(1);
  });
});

describe("NotificationUtils.dismissNotification", () => {
  beforeEach(resetState);

  it("removes the notification ID from activeNotifications immediately", () => {
    const notification = document.createElement("div");
    notification.className = "notification-ping";
    const id = "test-notification-abc";
    notification.setAttribute("data-notification-id", id);
    NotificationUtils.activeNotifications.add(id);
    document.body.appendChild(notification);

    NotificationUtils.dismissNotification(notification);

    expect(NotificationUtils.activeNotifications.has(id)).toBe(false);
  });

  it("adds the dismissing CSS class to the notification", () => {
    const notification = document.createElement("div");
    notification.className = "notification-ping";
    notification.setAttribute("data-notification-id", "some-id");
    document.body.appendChild(notification);

    NotificationUtils.dismissNotification(notification);

    expect(notification.classList.contains("dismissing")).toBe(true);
  });

  it("does nothing when the notification has no parent node", () => {
    const detached = document.createElement("div");
    expect(() => NotificationUtils.dismissNotification(detached)).not.toThrow();
  });
});
