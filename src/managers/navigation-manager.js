import { TimeUtils } from "../utils/common-utils.js";
import { TagStatistics } from "../utils/tag-statistics.js";
import { logger } from "../utils/logger.js";

export class NavigationManager {
  constructor() {
    this.currentView = "timer";
    this.initialized = false;
    this.currentTooltip = null; // Track current tooltip for proper cleanup
    this.tooltipTimeout = null; // Track timeout for debounced tooltip removal
    this.tagStatistics = new TagStatistics();

    // Apply timer-active class on initial load since default view is timer
    document.body.classList.add("timer-active");
    document.documentElement.classList.add("timer-active");
  }

  async init() {
    if (this.initialized) {
      logger.debug("NavigationManager already initialized, skipping...");
      return;
    }

    this.initialized = true;
    logger.info("Initializing NavigationManager...");

    const navButtons = document.querySelectorAll(".sidebar-icon, .sidebar-icon-large");
    navButtons.forEach((btn) => {
      btn.removeEventListener("click", this.handleNavClick);
      btn.addEventListener("click", this.handleNavClick.bind(this));
    });

    await this.initCalendar();
    await this.initSessionsTable();
  }

  /** @param {any} e */
  async handleNavClick(e) {
    const view = /** @type {HTMLElement} */ (e.currentTarget).dataset.view;
    await this.switchView(view);
  }

  /** @param {any} view */
  async switchView(view) {
    document.querySelectorAll(".sidebar-icon, .sidebar-icon-large").forEach((btn) => {
      btn.classList.remove("active");
    });
    const viewEl = document.querySelector(`[data-view="${view}"]`);
    if (viewEl) {
      viewEl.classList.add("active");
    }

    document.querySelectorAll(".view-container").forEach((container) => {
      container.classList.add("hidden");
    });

    const viewContainer = document.getElementById(`${view}-view`);
    if (viewContainer) {
      viewContainer.classList.remove("hidden");
    }
    this.currentView = view;

    const body = document.body;
    const html = document.documentElement;
    if (view === "timer") {
      body.classList.add("timer-active");
      html.classList.add("timer-active");
      if (window.pomodoroTimer) {
        window.pomodoroTimer.updateDisplay();
      }
    } else {
      body.classList.remove("timer-active", "focus", "break", "longBreak");
      html.classList.remove("timer-active");
    }

    if (view === "calendar") {
      await this.updateCalendar();
      this.updateWeekDisplay();
      await this.updateFocusSummary();
      await this.updateWeeklySessionsChart();
      this.updateDailyChart(this.selectedDate || this.currentDate);
      await this.updateTagUsageChart();
      await this.updateSelectedDayDetails(this.selectedDate || this.currentDate);
      await this.initSessionsTable(this.selectedDate || this.currentDate);
    } else if (view === "settings") {
      if (window.settingsManager) {
        window.settingsManager.populateSettingsUI();
      }
    } else if (view === "team") {
      if (window.teamManager) {
        await window.teamManager.init();
      }
    }
  }

  async initCalendar() {
    const prevBtn = document.getElementById("prev-month");
    const nextBtn = document.getElementById("next-month");
    const prevWeekBtn = document.getElementById("prev-week");
    const nextWeekBtn = document.getElementById("next-week");

    this.currentDate = new Date();
    this.displayMonth = new Date(this.currentDate);
    this.selectedWeek = this.getWeekStart(this.currentDate);

    if (prevBtn) {
      prevBtn.addEventListener("click", async () => {
        if (this.displayMonth) {
          this.displayMonth.setMonth(this.displayMonth.getMonth() - 1);
        }
        await this.updateCalendar();
      });
    }

    if (nextBtn) {
      nextBtn.addEventListener("click", async () => {
        if (this.displayMonth) {
          this.displayMonth.setMonth(this.displayMonth.getMonth() + 1);
        }
        await this.updateCalendar();
      });
    }

    if (prevWeekBtn) {
      prevWeekBtn.addEventListener("click", async () => {
        if (this.selectedWeek) {
          this.selectedWeek.setDate(this.selectedWeek.getDate() - 7);
        }
        this.updateWeekDisplay();
        await this.updateFocusSummary();
        await this.updateWeeklySessionsChart();
        this.updateDailyChart();
        await this.updateTagUsageChart();
      });
    }

    if (nextWeekBtn) {
      nextWeekBtn.addEventListener("click", async () => {
        if (this.selectedWeek) {
          this.selectedWeek.setDate(this.selectedWeek.getDate() + 7);
        }
        this.updateWeekDisplay();
        await this.updateFocusSummary();
        await this.updateWeeklySessionsChart();
        this.updateDailyChart();
        await this.updateTagUsageChart();
      });
    }

    // Initial updates will be handled by switchView when calendar is shown
  }

  /** @param {any} date */
  getWeekStart(date) {
    return TimeUtils.getWeekStart(date);
  }

  updateWeekDisplay() {
    const weekRangeEl = document.getElementById("week-range");
    if (!weekRangeEl) {
      return;
    }
    const weekStart = new Date(this.selectedWeek || new Date());
    const weekEnd = new Date(weekStart);
    weekEnd.setDate(weekEnd.getDate() + 6);

    weekRangeEl.textContent = TimeUtils.formatDateRange(weekStart, weekEnd);
  }

  async updateFocusSummary() {
    const totalFocusWeekEl = document.getElementById("total-focus-week");
    const totalFocusChangeEl = document.getElementById("total-focus-change");
    const avgFocusDayEl = document.getElementById("avg-focus-day");
    const avgFocusChangeEl = document.getElementById("avg-focus-change");
    const weeklySessionsEl = document.getElementById("weekly-sessions");
    const weeklySessionsChangeEl = document.getElementById("weekly-sessions-change");
    const weeklyFocusTimeEl = document.getElementById("weekly-focus-time");
    const weeklyFocusChangeEl = document.getElementById("weekly-focus-change");

    let avgFocus = 0;
    let weeklyFocusTime = 0;
    let weeklySessions = 0;
    let previousWeekAvgFocus = 0;
    let previousWeekFocusTime = 0;
    let previousWeeklySessions = 0;

    try {
      const weekStart = new Date(this.selectedWeek || this.getWeekStart(this.currentDate));
      const previousWeekStart = new Date(weekStart);
      previousWeekStart.setDate(weekStart.getDate() - 7);

      let weekTotal = 0;
      let daysWithData = 0;

      for (let i = 0; i < 7; i++) {
        const date = new Date(weekStart);
        date.setDate(weekStart.getDate() + i);

        let dayTotalTime = 0;
        let daySessions = 0;

        if (window.sessionManager) {
          const sessions = window.sessionManager.getSessionsForDate(date);
          const focusSessions = sessions.filter((/** @type {any} */ s) =>
            this.isFocusOrCustomSession(s)
          );

          dayTotalTime = focusSessions.reduce(
            (/** @type {any} */ total, /** @type {any} */ session) =>
              total + (session.duration || 0) * 60,
            0
          );
          daySessions = focusSessions.length;
        }

        if (dayTotalTime > 0) {
          weekTotal += dayTotalTime;
          weeklyFocusTime += dayTotalTime;
          weeklySessions += daySessions;
          daysWithData++;
        }
      }

      avgFocus = daysWithData > 0 ? weekTotal / daysWithData : 0;

      let previousWeekTotal = 0;
      let previousDaysWithData = 0;

      for (let i = 0; i < 7; i++) {
        const date = new Date(previousWeekStart);
        date.setDate(previousWeekStart.getDate() + i);

        let dayTotalTime = 0;
        let daySessions = 0;

        if (window.sessionManager) {
          const sessions = window.sessionManager.getSessionsForDate(date);
          const focusSessions = sessions.filter((/** @type {any} */ s) =>
            this.isFocusOrCustomSession(s)
          );

          dayTotalTime = focusSessions.reduce(
            (/** @type {any} */ total, /** @type {any} */ session) =>
              total + (session.duration || 0) * 60,
            0
          );
          daySessions = focusSessions.length;
        }

        if (dayTotalTime > 0) {
          previousWeekTotal += dayTotalTime;
          previousWeekFocusTime += dayTotalTime;
          previousWeeklySessions += daySessions;
          previousDaysWithData++;
        }
      }

      previousWeekAvgFocus =
        previousDaysWithData > 0 ? previousWeekTotal / previousDaysWithData : 0;
    } catch (error) {
      logger.error("Failed to load weekly data:", error);
    }

    const weeklyFocusChange = this.calculatePercentageChange(
      weeklyFocusTime,
      previousWeekFocusTime
    );
    const avgFocusChange = this.calculatePercentageChange(avgFocus, previousWeekAvgFocus);
    const weeklySessionsChange = this.calculatePercentageChange(
      weeklySessions,
      previousWeeklySessions
    );

    if (totalFocusWeekEl) {
      totalFocusWeekEl.textContent = TimeUtils.formatTime(weeklyFocusTime);
    }
    this.updateChangeElement(totalFocusChangeEl, weeklyFocusChange);

    if (avgFocusDayEl) {
      avgFocusDayEl.textContent = TimeUtils.formatTime(avgFocus);
    }
    this.updateChangeElement(avgFocusChangeEl, avgFocusChange);

    if (weeklySessionsEl) {
      weeklySessionsEl.textContent = weeklySessions.toString();
    }
    this.updateChangeElement(weeklySessionsChangeEl, weeklySessionsChange);

    if (weeklyFocusTimeEl) {
      weeklyFocusTimeEl.textContent = TimeUtils.formatTime(weeklyFocusTime);
    }
    this.updateChangeElement(weeklyFocusChangeEl, weeklyFocusChange);
  }

  /** @param {any} session */
  isFocusOrCustomSession(session) {
    const type = session.session_type || session.type;
    return type === "focus" || type === "custom";
  }

  /**
   * @param {any} current
   * @param {any} previous
   */
  calculatePercentageChange(current, previous) {
    if (previous === 0) {
      return current > 0 ? 100 : 0;
    }
    return Math.round(((current - previous) / previous) * 100);
  }

  /**
   * @param {any} element
   * @param {any} change
   */
  updateChangeElement(element, change) {
    if (!element) {
      return;
    }
    element.classList.remove("positive", "negative", "neutral");

    const icon = element.querySelector("i");
    const span = element.querySelector("span");
    if (!icon || !span) {
      return;
    }

    if (change > 0) {
      span.textContent = `+${change}%`;
      icon.className = "ri-arrow-up-line";
      element.classList.add("positive");
    } else if (change < 0) {
      span.textContent = `${change}%`;
      icon.className = "ri-arrow-down-line";
      element.classList.add("negative");
    } else {
      span.textContent = "0%";
      icon.className = "ri-subtract-line";
      element.classList.add("neutral");
    }
  }

  async updateDailyChart(date = this.selectedDate || this.currentDate || new Date()) {
    const dailyChart = document.getElementById("daily-chart");
    if (!dailyChart) {
      return;
    }

    dailyChart.innerHTML = "";

    const hours = Array.from({ length: 24 }, (_, i) => i);
    const maxHeight = 140; // Increased height to use more of available space

    try {
      const todaysSessions = window.sessionManager
        ? window.sessionManager.getSessionsForDate(date)
        : [];

      const hourlyData = hours.map((hour) => ({
        hour,
        focusMinutes: 0,
        breakMinutes: 0,
      }));

      // Process all sessions with unified logic.
      // All stored sessions are focus sessions; break and longBreak
      // sessions are no longer recorded.
      todaysSessions.forEach((/** @type {any} */ session) => {
        const [startHour, startMinute] = session.start_time.split(":").map(Number);
        const [endHour, endMinute] = session.end_time.split(":").map(Number);

        const startTotalMinutes = startHour * 60 + startMinute;
        const endTotalMinutes = endHour * 60 + endMinute;

        // Distribute session time across all affected hours
        for (let hour = startHour; hour <= endHour; hour++) {
          const hourStartMinutes = hour * 60;
          const hourEndMinutes = (hour + 1) * 60;

          const sessionStartInHour = Math.max(startTotalMinutes, hourStartMinutes);
          const sessionEndInHour = Math.min(endTotalMinutes, hourEndMinutes);

          if (sessionEndInHour > sessionStartInHour) {
            const minutesInThisHour = sessionEndInHour - sessionStartInHour;
            hourlyData[hour].focusMinutes += minutesInThisHour;
          }
        }
      });

      // Find max total minutes for scaling (only focus minutes now)
      const maxTotalMinutes = Math.max(
        ...hourlyData.map((data) => data.focusMinutes),
        60 // Minimum scale of 1 hour
      );

      hours.forEach((hour) => {
        const data = hourlyData[hour];
        const totalMinutes = data.focusMinutes; // Only focus minutes now

        const hourBar = document.createElement("div");
        hourBar.className = "hour-bar";

        const height =
          totalMinutes > 0 ? Math.max((totalMinutes / maxTotalMinutes) * maxHeight, 8) : 8; // Minimum height for visibility

        hourBar.style.height = `${height}px`;

        if (totalMinutes > 0) {
          const focusSegment = document.createElement("div");
          focusSegment.className = "hour-bar-focus";
          focusSegment.style.height = "100%"; // Full height since only focus
          hourBar.appendChild(focusSegment);
        } else {
          hourBar.classList.add("hour-bar-empty");
        }

        const hourLabel = document.createElement("div");
        hourLabel.className = "hour-label";
        hourLabel.textContent = hour.toString().padStart(2, "0");
        hourBar.appendChild(hourLabel);

        const focusText = data.focusMinutes > 0 ? `${data.focusMinutes}m focus` : "";
        const activityText = focusText || "No activity";

        // Use custom tooltip instead of native title
        hourBar.dataset.tooltip = `${hour}:00 - ${activityText}`;

        this.addTooltipEvents(hourBar);

        hourBar.dataset.hour = String(hour);
        hourBar.dataset.focusMinutes = String(data.focusMinutes);

        dailyChart.appendChild(hourBar);
      });

      dailyChart.addEventListener("mouseleave", () => {
        this.removeTooltip();
      });
    } catch (error) {
      logger.error("Failed to load daily chart data:", error);

      hours.forEach((hour) => {
        const hourBar = document.createElement("div");
        hourBar.className = "hour-bar hour-bar-empty";
        hourBar.style.height = "8px";

        const hourLabel = document.createElement("div");
        hourLabel.className = "hour-label";
        hourLabel.textContent = hour.toString().padStart(2, "0");
        hourBar.appendChild(hourLabel);

        hourBar.dataset.tooltip = `${hour}:00 - No data available`;
        this.addTooltipEvents(hourBar);
        dailyChart.appendChild(hourBar);
      });
    }
  }

  async updateWeeklySessionsChart() {
    const weeklyChart = document.getElementById("weekly-chart");
    if (!weeklyChart) {
      return;
    }
    weeklyChart.innerHTML = "";

    const days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    const maxHeight = 70;

    try {
      const weekStart = new Date(this.selectedWeek || this.getWeekStart(this.currentDate));
      const today = new Date();

      // First pass: collect all session data for the week to determine max value for scaling
      /** @type {any[]} */
      const weekData = [];
      let maxSessionsMinutes = 0;

      days.forEach((day, index) => {
        const date = new Date(weekStart);
        date.setDate(weekStart.getDate() + index);

        let sessionsMinutes = 0;
        let sessions = 0;

        if (window.sessionManager) {
          const allSessions = window.sessionManager.getSessionsForDate(date);
          const focusSessions = allSessions.filter((/** @type {any} */ s) =>
            this.isFocusOrCustomSession(s)
          );

          sessionsMinutes = focusSessions.reduce(
            (/** @type {any} */ total, /** @type {any} */ session) =>
              total + (session.duration || 0),
            0
          );
          sessions = focusSessions.length;
        }

        weekData.push({
          day,
          date,
          sessionsMinutes,
          sessions,
          isPast: date <= today,
        });

        if (sessionsMinutes > maxSessionsMinutes) {
          maxSessionsMinutes = sessionsMinutes;
        }
      });

      // Calculate average daily session time (include all days with data, including today)
      let avgSessionTime = 0;
      let totalDailyTime = 0;
      let daysWithSessions = 0;

      weekData.forEach(({ sessionsMinutes }) => {
        if (sessionsMinutes > 0) {
          totalDailyTime += sessionsMinutes;
          daysWithSessions++;
        }
      });

      avgSessionTime = daysWithSessions > 0 ? totalDailyTime / daysWithSessions : 0;

      // Use a minimum baseline for maxSessionsMinutes to avoid tiny bars
      const scalingMax = Math.max(maxSessionsMinutes, Math.max(avgSessionTime, 60)); // Include average in scaling

      if (avgSessionTime > 0 && daysWithSessions > 0) {
        const avgLine = document.createElement("div");
        avgLine.className = "week-average-line";

        const avgLineHeight = (avgSessionTime / scalingMax) * maxHeight;
        avgLine.style.bottom = `${avgLineHeight}px`;
        avgLine.style.left = "0";
        avgLine.style.right = "0";
        avgLine.style.position = "absolute";
        avgLine.style.height = "1px"; // Reduced thickness for dashed line
        avgLine.style.backgroundColor = "transparent";
        avgLine.style.borderTop = "1px dashed #d1d5db"; // Light gray dashed line
        avgLine.style.zIndex = "10";
        avgLine.style.opacity = "0.6";

        const avgLabel = document.createElement("div");
        avgLabel.className = "week-average-label";
        avgLabel.textContent = `Avg: ${Math.round(avgSessionTime)}m`;
        avgLabel.style.position = "absolute";
        avgLabel.style.right = "5px";
        avgLabel.style.top = "-18px";
        avgLabel.style.fontSize = "10px";
        avgLabel.style.color = "#9ca3af"; // Light gray to match dashed line
        avgLabel.style.fontWeight = "500";
        avgLabel.style.background = "white";
        avgLabel.style.padding = "1px 4px";
        avgLabel.style.borderRadius = "3px";
        avgLabel.style.whiteSpace = "nowrap";
        avgLabel.style.opacity = "0.8";

        avgLine.appendChild(avgLabel);

        // Set relative positioning on chart to contain the absolute line
        weeklyChart.style.position = "relative";
        weeklyChart.appendChild(avgLine);
      }

      weekData.forEach(({ day, sessionsMinutes, sessions, isPast }) => {
        const dayBar = document.createElement("div");
        dayBar.className = "week-day-bar";

        const height =
          sessionsMinutes > 0 ? Math.max((sessionsMinutes / scalingMax) * maxHeight, 8) : 8;

        dayBar.style.height = `${height}px`;

        if (isPast && sessions > 0) {
          dayBar.style.borderTop = "1px solid #d1d5db"; // Light gray to match dashed line
        }

        if (sessionsMinutes > 0) {
          const valueLabel = document.createElement("div");
          valueLabel.className = "week-day-bar-value";
          valueLabel.textContent = `${sessions}`;
          dayBar.appendChild(valueLabel);
        }

        const hours = Math.floor(sessionsMinutes / 60);
        const minutes = Math.floor(sessionsMinutes % 60);
        const timeText = hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
        const avgPerSession = sessions > 0 ? Math.round(sessionsMinutes / sessions) : 0;
        const tooltipText =
          sessions > 0
            ? `${day}: ${timeText} (${sessions} sessions, ${avgPerSession}m avg/session)`
            : `${day}: ${timeText} (${sessions} sessions)`;

        // Use custom tooltip instead of native title
        dayBar.dataset.tooltip = tooltipText;

        this.addTooltipEvents(dayBar);

        weeklyChart.appendChild(dayBar);
      });

      weeklyChart.addEventListener("mouseleave", () => {
        this.removeTooltip();
      });
    } catch (error) {
      logger.error("Failed to load weekly chart data:", error);
      days.forEach((day, _index) => {
        const dayBar = document.createElement("div");
        dayBar.className = "week-day-bar";
        dayBar.style.height = "8px";

        // Use custom tooltip instead of native title
        dayBar.dataset.tooltip = `${day}: No data available`;

        this.addTooltipEvents(dayBar);

        weeklyChart.appendChild(dayBar);
      });
    }
  }

  async updateTagUsageChart() {
    try {
      const tags = window.tagManager ? window.tagManager.tags : [];

      const sessions = [];
      const startOfWeek = new Date(this.selectedWeek || this.getWeekStart(this.currentDate));

      for (let i = 0; i < 7; i++) {
        const date = new Date(startOfWeek);
        date.setDate(startOfWeek.getDate() + i);

        if (window.sessionManager) {
          const dailySessions = window.sessionManager.getSessionsForDate(date);

          // Filter to focus sessions only and add date info
          const focusSessions = dailySessions
            .filter((/** @type {any} */ s) => this.isFocusOrCustomSession(s))
            .map((/** @type {any} */ session) => ({
              ...session,
              date: date.toISOString().split("T")[0], // Add date in YYYY-MM-DD format
            }));
          sessions.push(...focusSessions);
        }
      }

      const tagStatsData = this.tagStatistics.getCurrentWeekTagStats(sessions, tags);

      this.tagStatistics.renderTagPieChart("tag-pie-chart", "tag-legend", tagStatsData);
    } catch (error) {
      logger.error("Error updating tag usage chart:", error);

      const chartContainer = document.getElementById("tag-pie-chart");
      const legendContainer = document.getElementById("tag-legend");

      if (chartContainer) {
        chartContainer.innerHTML = `
                    <div class="pie-chart-placeholder">
                        <i class="ri-pie-chart-line"></i>
                        <span>Error loading data</span>
                    </div>
                `;
      }

      if (legendContainer) {
        legendContainer.innerHTML = "";
      }
    }
  }

  async updateSelectedDayDetails(date = this.currentDate || new Date()) {
    const selectedDayTitle = document.getElementById("selected-day-title");
    const timelineTrack = document.getElementById("timeline-track");
    const timelineHours = document.getElementById("timeline-hours");

    const dateStr = date.toLocaleDateString("en-US", {
      weekday: "long",
      year: "numeric",
      month: "long",
      day: "numeric",
    });

    const isToday = this.isSameDay(date, new Date());
    if (selectedDayTitle) {
      selectedDayTitle.textContent = isToday ? "Today's Sessions" : `${dateStr} Sessions`;
    }

    this.setupTimelineHours(timelineHours);

    if (!timelineTrack) {
      return;
    }
    timelineTrack.innerHTML = "";
    timelineTrack.style.height = "50px";

    try {
      let allSessions = [];
      if (window.sessionManager) {
        allSessions = window.sessionManager.getSessionsForDate(date);
      }

      allSessions.sort((/** @type {any} */ a, /** @type {any} */ b) =>
        a.start_time.localeCompare(b.start_time)
      );

      if (allSessions.length === 0) {
        const noSessions = document.createElement("div");
        noSessions.className = "timeline-empty";
        noSessions.textContent = "No sessions completed";
        timelineTrack.appendChild(noSessions);
        return;
      }

      // Filter out break sessions from timeline display
      const visibleSessions = allSessions.filter((/** @type {any} */ session) =>
        this.isFocusOrCustomSession(session)
      );

      // Create timeline session blocks (excluding break sessions)
      visibleSessions.forEach((/** @type {any} */ session) => {
        this.createTimelineSession(session, date, timelineTrack, visibleSessions);
      });

      this.updateTimelineHeight(timelineTrack, visibleSessions.length);

      this.initializeTimelineInteractions();
    } catch (error) {
      logger.error("Failed to load session details:", error);
      const errorItem = document.createElement("div");
      errorItem.className = "timeline-empty";
      errorItem.textContent = "Error loading session data";
      timelineTrack.appendChild(errorItem);
    }
  }

  async updateCalendar() {
    const calendarGrid = document.getElementById("calendar-grid");
    const currentMonthEl = document.getElementById("current-month");
    if (!calendarGrid || !this.displayMonth) {
      return;
    }

    const monthNames = [
      "January",
      "February",
      "March",
      "April",
      "May",
      "June",
      "July",
      "August",
      "September",
      "October",
      "November",
      "December",
    ];
    if (currentMonthEl) {
      currentMonthEl.textContent = `${monthNames[this.displayMonth.getMonth()]} ${this.displayMonth.getFullYear()}`;
    }

    calendarGrid.innerHTML = "";

    const dayHeaders = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    dayHeaders.forEach((day) => {
      const dayEl = document.createElement("div");
      dayEl.className = "calendar-day day-name";
      dayEl.textContent = day;
      calendarGrid.appendChild(dayEl);
    });

    const firstDay = new Date(this.displayMonth.getFullYear(), this.displayMonth.getMonth(), 1);
    const lastDay = new Date(this.displayMonth.getFullYear(), this.displayMonth.getMonth() + 1, 0);
    const daysInMonth = lastDay.getDate();
    const startingDay = firstDay.getDay();

    for (let i = 0; i < startingDay; i++) {
      const emptyDay = document.createElement("div");
      emptyDay.className = "calendar-day";
      calendarGrid.appendChild(emptyDay);
    }

    for (let day = 1; day <= daysInMonth; day++) {
      const dayEl = document.createElement("div");
      dayEl.className = "calendar-day";

      const dayNumber = document.createElement("div");
      dayNumber.className = "calendar-day-number";
      dayNumber.textContent = String(day);
      dayEl.appendChild(dayNumber);

      const dayDate = new Date(this.displayMonth.getFullYear(), this.displayMonth.getMonth(), day);
      if (this.isSameDay(dayDate, this.currentDate)) {
        dayEl.classList.add("today");
      }
      if (this.selectedDate && this.isSameDay(dayDate, this.selectedDate)) {
        dayEl.classList.add("selected");
      }

      const dots = document.createElement("div");
      dots.className = "calendar-day-dots";

      if (window.sessionManager) {
        const sessions = window.sessionManager.getSessionsForDate(dayDate);
        const focusSessions = sessions.filter((/** @type {any} */ s) =>
          this.isFocusOrCustomSession(s)
        );

        if (focusSessions.length > 0) {
          dayEl.classList.add("has-sessions");
          const numDots = Math.min(focusSessions.length, 5); // Max 5 dots
          for (let i = 0; i < numDots; i++) {
            const dot = document.createElement("div");
            dot.className = "calendar-dot";
            dots.appendChild(dot);
          }
        }
      }

      dayEl.appendChild(dots);

      dayEl.addEventListener("click", async (e) => {
        await this.selectDay(dayDate, e.currentTarget);
      });

      calendarGrid.appendChild(dayEl);
    }
  }

  /**
   * @param {any} date1
   * @param {any} date2
   */
  isSameDay(date1, date2) {
    return TimeUtils.isSameDay(date1, date2);
  }

  /**
   * @param {any} date
   * @param {any} dayEl
   */
  async selectDay(date, dayEl) {
    document.querySelectorAll(".calendar-day").forEach((day) => {
      day.classList.remove("selected");
    });

    dayEl?.classList.add("selected");

    this.selectedDate = date;
    this.selectedWeek = this.getWeekStart(date);
    this.updateWeekDisplay();
    await this.updateSelectedDayDetails(date);
    await this.updateFocusSummary();
    await this.updateWeeklySessionsChart();
    this.updateDailyChart(date);
    await this.updateTagUsageChart();
    await this.populateSessionsTableForDate(date);
  }

  /** @param {any} seconds */
  formatTime(seconds) {
    return TimeUtils.formatTime(seconds);
  }

  /**
   * @param {any} timelineTrack
   * @param {any} totalSessions
   */
  updateTimelineHeight(timelineTrack, totalSessions) {
    const rowHeight = 20; // Spacing between rows
    const sessionHeight = 15; // Height of each session
    const topPadding = 10;
    const bottomPadding = 10;
    const minHeight = 60; // Minimum height even with no sessions

    if (totalSessions === 0) {
      timelineTrack.style.height = `${minHeight}px`;
    } else {
      const lastRowBottom = (totalSessions - 1) * rowHeight + sessionHeight;
      const requiredHeight = topPadding + lastRowBottom + bottomPadding;
      logger.debug(
        `Timeline height calculation: ${topPadding} + ${lastRowBottom} + ${bottomPadding} = ${requiredHeight}px for ${totalSessions} sessions`
      );
      timelineTrack.style.height = `${requiredHeight}px`;
    }

    // Add vertical grid lines
    this.addTimelineGridLines(timelineTrack);
  }

  /** @param {any} timelineTrack */
  addTimelineGridLines(timelineTrack) {
    const existingLines = timelineTrack.querySelectorAll(".timeline-grid-line");
    existingLines.forEach((/** @type {any} */ line) => line.remove());

    const majorHours = [0, 4, 8, 12, 16, 20];
    const timelineStartHour = 0;
    const timelineRangeHours = 24;

    majorHours.forEach((hour) => {
      const line = document.createElement("div");
      line.className = "timeline-grid-line";

      const hoursFromStart = hour - timelineStartHour;
      const percentage = (hoursFromStart / timelineRangeHours) * 100;
      line.style.left = `${percentage}%`;

      timelineTrack.appendChild(line);
    });
  }

  /** @param {any} timelineHours */
  setupTimelineHours(timelineHours) {
    if (!timelineHours) {
      return;
    }
    timelineHours.innerHTML = "";

    const majorHours = [0, 4, 8, 12, 16, 20];
    const timelineStartHour = 0; // 12 AM (midnight)
    const timelineRangeHours = 24; // Full day = 24 hours

    majorHours.forEach((hour) => {
      const hourElement = document.createElement("div");
      hourElement.className = "timeline-hour";
      hourElement.textContent = `${hour.toString().padStart(2, "0")}:00`;

      const hoursFromStart = hour - timelineStartHour;
      const percentage = (hoursFromStart / timelineRangeHours) * 100;
      hourElement.style.left = `${percentage}%`;

      timelineHours.appendChild(hourElement);
    });
  }

  /**
   * @param {any} session
   * @param {any} date
   * @param {any} timelineTrack
   * @param {any[]} allSessions
   */
  createTimelineSession(session, date, timelineTrack, allSessions = []) {
    const sessionElement = document.createElement("div");
    sessionElement.className = `timeline-session focus`; // All sessions are focus sessions
    sessionElement.dataset.sessionId = session.id;

    const isToday = this.isSameDay(date, new Date());
    const sessionType = session.session_type || session.type || "Focus";

    const [startHour, startMinute] = session.start_time.split(":").map(Number);
    const [endHour, endMinute] = session.end_time.split(":").map(Number);

    // Calculate position and width (00:00 = 0%, 23:59 = 100%)
    const startTimeInMinutes = startHour * 60 + startMinute;
    const endTimeInMinutes = endHour * 60 + endMinute;
    const timelineStartMinutes = 0; // 00:00 (midnight)
    const timelineEndMinutes = 24 * 60; // 24:00 (next midnight)
    const timelineRangeMinutes = timelineEndMinutes - timelineStartMinutes;

    const leftPercent = Math.max(
      0,
      ((startTimeInMinutes - timelineStartMinutes) / timelineRangeMinutes) * 100
    );
    const rightPercent = Math.min(
      100,
      ((endTimeInMinutes - timelineStartMinutes) / timelineRangeMinutes) * 100
    );
    const widthPercent = rightPercent - leftPercent;

    sessionElement.style.left = `${leftPercent}%`;
    sessionElement.style.width = `${widthPercent}%`;

    if (isToday) {
      sessionElement.classList.add("today-session");
      sessionElement.innerHTML = `
        <div class="session-handle left"></div>
        <div class="timeline-session-content-minimal"></div>
        <div class="session-handle right"></div>
      `;

      // Use textContent-safe title (no innerHTML) for session data
      sessionElement.title = `Focus: ${session.start_time} - ${session.end_time} (${session.duration}m)`;
    } else {
      sessionElement.innerHTML = `
        <div class="session-handle left"></div>
        <div class="timeline-session-content">
          <span class="timeline-session-type"></span>
          <span class="timeline-session-time"></span>
        </div>
        <div class="session-handle right"></div>
      `;

      // Use textContent for session data (API-controlled) to prevent XSS
      const typeEl = sessionElement.querySelector(".timeline-session-type");
      const timeEl = sessionElement.querySelector(".timeline-session-time");
      if (typeEl) {
        typeEl.textContent = sessionType;
      }
      if (timeEl) {
        timeEl.textContent = `${session.start_time} - ${session.end_time}`;
      }
    }

    this.addTimelineSessionEventListeners(sessionElement, session, date);

    const offset = this.calculateSessionOffset(session, allSessions);
    sessionElement.style.transform = `translateY(${offset}px)`;
    if (offset > 0) {
      sessionElement.classList.add("session-stacked");
    }

    timelineTrack.appendChild(sessionElement);
  }

  /**
   * @param {any} sessionElement
   * @param {any} session
   * @param {any} date
   */
  addTimelineSessionEventListeners(sessionElement, session, date) {
    // Double-click to edit
    sessionElement.addEventListener("dblclick", (/** @type {any} */ e) => {
      e.preventDefault();
      if (window.sessionManager) {
        window.sessionManager.openEditSessionModal(session, date);
      }
    });

    // Right-click context menu
    sessionElement.addEventListener("contextmenu", (/** @type {any} */ e) => {
      e.preventDefault();
      this.showSessionContextMenu(e, session, date);
    });

    // Drag to move
    sessionElement.addEventListener("mousedown", (/** @type {any} */ e) => {
      // Don't start drag if clicking on resize handles
      if (
        /** @type {Element} */ (e.target).classList.contains("session-handle") ||
        /** @type {Element} */ (e.target).closest(".session-handle")
      ) {
        return;
      }
      this.startSessionDrag(e, sessionElement, session);
    });

    // Handle resize
    const leftHandle = sessionElement.querySelector(".session-handle.left");
    const rightHandle = sessionElement.querySelector(".session-handle.right");

    if (leftHandle) {
      leftHandle.addEventListener("mousedown", (/** @type {any} */ e) => {
        e.stopPropagation();
        this.startSessionResize(e, sessionElement, session, "left");
      });
    }

    if (rightHandle) {
      rightHandle.addEventListener("mousedown", (/** @type {any} */ e) => {
        e.stopPropagation();
        this.startSessionResize(e, sessionElement, session, "right");
      });
    }

    // Hover tooltip
    /** @type {any} */
    let hoverTooltip = null;

    sessionElement.addEventListener("mouseenter", (/** @type {any} */ e) => {
      // Don't show hover tooltip if dragging or resizing (they have their own tooltips)
      if (
        sessionElement.classList.contains("dragging") ||
        sessionElement.classList.contains("resizing")
      ) {
        return;
      }

      hoverTooltip = this.createSessionHoverTooltip(session);
      document.body.appendChild(hoverTooltip);
      this.updateHoverTooltip(hoverTooltip, e);
    });

    sessionElement.addEventListener("mousemove", (/** @type {any} */ e) => {
      if (
        hoverTooltip &&
        !sessionElement.classList.contains("dragging") &&
        !sessionElement.classList.contains("resizing")
      ) {
        this.updateHoverTooltip(hoverTooltip, e);
      }
    });

    sessionElement.addEventListener("mouseleave", () => {
      if (hoverTooltip && hoverTooltip.parentNode) {
        hoverTooltip.parentNode.removeChild(hoverTooltip);
        hoverTooltip = null;
      }
    });
  }

  initializeTimelineInteractions() {
    // Close context menu on click outside
    document.addEventListener("click", () => {
      const contextMenu = document.querySelector(".session-context-menu");
      if (contextMenu) {
        contextMenu.remove();
      }
    });
  }

  /**
   * @param {any} e
   * @param {any} session
   * @param {any} date
   */
  showSessionContextMenu(e, session, date) {
    // Remove existing context menu
    const existingMenu = document.querySelector(".session-context-menu");
    if (existingMenu) {
      existingMenu.remove();
    }

    const contextMenu = document.createElement("div");
    contextMenu.className = "session-context-menu";
    contextMenu.style.left = `${e.pageX}px`;
    contextMenu.style.top = `${e.pageY}px`;
    contextMenu.style.display = "block";

    contextMenu.innerHTML = `
      <div class="context-menu-item edit-item">Edit Session</div>
      <div class="context-menu-item danger delete-item">Delete</div>
    `;

    const editItem = contextMenu.querySelector(".edit-item");
    if (editItem) {
      editItem.addEventListener("click", () => {
        if (window.sessionManager) {
          window.sessionManager.openEditSessionModal(session, date);
        }
        contextMenu.remove();
      });
    }

    const deleteItem = contextMenu.querySelector(".delete-item");
    if (deleteItem) {
      deleteItem.addEventListener("click", () => {
        if (window.sessionManager) {
          window.sessionManager.currentEditingSession = session;
          window.sessionManager.selectedDate = date;
          window.sessionManager.deleteCurrentSession();
        }
        contextMenu.remove();
      });
    }

    document.body.appendChild(contextMenu);
  }

  /**
   * @param {any} e
   * @param {any} sessionElement
   * @param {any} session
   */
  startSessionDrag(e, sessionElement, session) {
    e.preventDefault();
    sessionElement.classList.add("dragging");

    const existingHoverTooltip = document.querySelector(".session-hover-tooltip");
    if (existingHoverTooltip && existingHoverTooltip.parentNode) {
      existingHoverTooltip.parentNode.removeChild(existingHoverTooltip);
    }

    const timeline = document.getElementById("timeline-track");
    if (!timeline) {
      return;
    }
    const timelineRect = timeline.getBoundingClientRect();

    const initialMouseX = e.clientX - timelineRect.left;
    const currentLeft = parseFloat(sessionElement.style.left) || 0;
    const currentLeftPx = (currentLeft / 100) * timelineRect.width;
    const offsetX = initialMouseX - currentLeftPx;

    const dragTooltip = this.createDragTimeTooltip();
    document.body.appendChild(dragTooltip);

    const handleMouseMove = (/** @type {any} */ e) => {
      const x = e.clientX - timelineRect.left - offsetX;
      const sessionWidth = parseFloat(sessionElement.style.width) || 0;
      const maxLeft = 100 - sessionWidth; // Prevent session from going beyond timeline
      const percentage = Math.max(0, Math.min(maxLeft, (x / timelineRect.width) * 100));
      sessionElement.style.left = `${percentage}%`;

      this.updateDragTooltip(dragTooltip, e, percentage, session);
    };

    const handleMouseUp = () => {
      sessionElement.classList.remove("dragging");
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);

      if (dragTooltip && dragTooltip.parentNode) {
        dragTooltip.parentNode.removeChild(dragTooltip);
      }

      this.updateSessionTimeFromPosition(sessionElement, session);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  }

  /**
   * @param {any} e
   * @param {any} sessionElement
   * @param {any} session
   * @param {any} side
   */
  startSessionResize(e, sessionElement, session, side) {
    e.preventDefault();
    sessionElement.classList.add("resizing");

    const existingHoverTooltip = document.querySelector(".session-hover-tooltip");
    if (existingHoverTooltip && existingHoverTooltip.parentNode) {
      existingHoverTooltip.parentNode.removeChild(existingHoverTooltip);
    }

    const timeline = document.getElementById("timeline-track");
    if (!timeline) {
      return;
    }
    const timelineRect = timeline.getBoundingClientRect();

    const resizeTooltip = this.createDragTimeTooltip();
    document.body.appendChild(resizeTooltip);

    const handleMouseMove = (/** @type {any} */ e) => {
      const x = e.clientX - timelineRect.left;
      const percentage = Math.max(0, Math.min(100, (x / timelineRect.width) * 100));

      const currentLeft = parseFloat(sessionElement.style.left);
      const currentWidth = parseFloat(sessionElement.style.width);
      const currentRight = currentLeft + currentWidth;

      if (side === "left") {
        const newLeft = Math.min(percentage, currentRight - 2); // Minimum 2% width
        const newWidth = currentRight - newLeft;
        sessionElement.style.left = `${newLeft}%`;
        sessionElement.style.width = `${newWidth}%`;
      } else {
        const newWidth = Math.max(2, percentage - currentLeft); // Minimum 2% width
        sessionElement.style.width = `${newWidth}%`;
      }

      this.updateResizeTooltip(resizeTooltip, e, sessionElement);
    };

    const handleMouseUp = () => {
      sessionElement.classList.remove("resizing");
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);

      if (resizeTooltip && resizeTooltip.parentNode) {
        resizeTooltip.parentNode.removeChild(resizeTooltip);
      }

      this.updateSessionTimeFromPosition(sessionElement, session);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  }

  /**
   * @param {any} sessionElement
   * @param {any} session
   */
  updateSessionTimeFromPosition(sessionElement, session) {
    const leftPercent = parseFloat(sessionElement.style.left);
    const widthPercent = parseFloat(sessionElement.style.width);
    const rightPercent = leftPercent + widthPercent;

    // Convert percentages back to time (00:00 to 23:59 range)
    const timelineStartMinutes = 0; // 00:00 (midnight)
    const timelineRangeMinutes = 24 * 60; // 24 hours (full day)

    const startMinutes = timelineStartMinutes + (leftPercent / 100) * timelineRangeMinutes;
    const endMinutes = timelineStartMinutes + (rightPercent / 100) * timelineRangeMinutes;

    const roundedStartMinutes = Math.max(0, Math.min(23 * 60 + 58, Math.round(startMinutes)));
    const roundedEndMinutes = Math.max(
      roundedStartMinutes + 1,
      Math.min(23 * 60 + 59, Math.round(endMinutes))
    );

    const startHour = Math.floor(roundedStartMinutes / 60);
    const startMin = roundedStartMinutes % 60;
    const endHour = Math.floor(roundedEndMinutes / 60);
    const endMin = roundedEndMinutes % 60;

    const newStartTime = `${startHour.toString().padStart(2, "0")}:${startMin.toString().padStart(2, "0")}`;
    const newEndTime = `${endHour.toString().padStart(2, "0")}:${endMin.toString().padStart(2, "0")}`;
    const newDuration = roundedEndMinutes - roundedStartMinutes;

    session.start_time = newStartTime;
    session.end_time = newEndTime;
    session.duration = newDuration;

    const timeDisplay = sessionElement.querySelector(".timeline-session-time");
    if (timeDisplay) {
      timeDisplay.textContent = `${newStartTime} - ${newEndTime}`;
    }

    if (sessionElement.classList.contains("today-session")) {
      sessionElement.title = `Focus: ${newStartTime} - ${newEndTime} (${newDuration}m)`;
    }

    if (window.sessionManager) {
      window.sessionManager.selectedDate = this.currentDate;
      window.sessionManager.updateSession(session);
    }
  }

  /** @param {any} element */
  addTooltipEvents(element) {
    element.addEventListener("mouseenter", () => {
      const tooltipText = element.dataset.tooltip;
      if (!tooltipText) {
        return;
      }

      // Clear any pending tooltip removal
      if (this.tooltipTimeout) {
        clearTimeout(this.tooltipTimeout);
        this.tooltipTimeout = null;
      }

      this.removeTooltip();

      const tooltipElement = document.createElement("div");
      tooltipElement.className = "custom-tooltip";
      tooltipElement.textContent = tooltipText;

      const rect = element.getBoundingClientRect();
      const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
      const scrollLeft = window.pageXOffset || document.documentElement.scrollLeft;

      tooltipElement.style.position = "absolute";
      tooltipElement.style.left = `${rect.left + scrollLeft + rect.width / 2}px`;
      tooltipElement.style.top = `${rect.top + scrollTop - 10}px`;
      tooltipElement.style.transform = "translateX(-50%) translateY(-100%)";
      tooltipElement.style.backgroundColor = "#1f2937"; // Changed to flat dark gray
      tooltipElement.style.color = "white";
      tooltipElement.style.padding = "8px 12px";
      tooltipElement.style.borderRadius = "6px";
      tooltipElement.style.fontSize = "0.75rem";
      tooltipElement.style.fontWeight = "500";
      tooltipElement.style.whiteSpace = "nowrap";
      tooltipElement.style.zIndex = "10000";
      tooltipElement.style.pointerEvents = "none";
      tooltipElement.style.boxShadow = "0 4px 12px rgba(0, 0, 0, 0.3)";
      tooltipElement.style.opacity = "0";
      tooltipElement.style.transition = "opacity 0.2s ease";

      // Add arrow
      const arrow = document.createElement("div");
      arrow.style.position = "absolute";
      arrow.style.top = "100%";
      arrow.style.left = "50%";
      arrow.style.transform = "translateX(-50%)";
      arrow.style.width = "0";
      arrow.style.height = "0";
      arrow.style.borderLeft = "5px solid transparent";
      arrow.style.borderRight = "5px solid transparent";
      arrow.style.borderTop = "5px solid #1f2937"; // Changed to match flat background
      tooltipElement.appendChild(arrow);

      document.body.appendChild(tooltipElement);

      // Store reference to current tooltip for cleanup
      this.currentTooltip = tooltipElement;

      // Fade in
      requestAnimationFrame(() => {
        if (tooltipElement.parentNode) {
          tooltipElement.style.opacity = "1";
        }
      });
    });

    element.addEventListener("mouseleave", () => {
      // Add slight delay to prevent flicker when moving between adjacent elements
      this.tooltipTimeout = setTimeout(() => {
        this.removeTooltip();
      }, 50);
    });
  }

  removeTooltip() {
    // Clear any pending timeout
    if (this.tooltipTimeout) {
      clearTimeout(this.tooltipTimeout);
      this.tooltipTimeout = null;
    }

    // Use stored reference first, then fallback to querySelector
    if (this.currentTooltip && this.currentTooltip.parentNode) {
      this.currentTooltip.style.opacity = "0";
      setTimeout(() => {
        if (this.currentTooltip && this.currentTooltip.parentNode) {
          this.currentTooltip.parentNode.removeChild(this.currentTooltip);
        }
        this.currentTooltip = null;
      }, 200);
      return;
    }

    // Fallback: remove any remaining tooltips
    const existingTooltips = document.querySelectorAll(".custom-tooltip");
    existingTooltips.forEach((tooltip) => {
      /** @type {HTMLElement} */ (tooltip).style.opacity = "0";
      setTimeout(() => {
        if (tooltip.parentNode) {
          tooltip.parentNode.removeChild(tooltip);
        }
      }, 200);
    });

    this.currentTooltip = null;
  }

  createDragTimeTooltip() {
    const tooltip = document.createElement("div");
    tooltip.className = "drag-time-tooltip";
    tooltip.style.cssText = `
            position: fixed;
            background: var(--shared-text);
            color: var(--card-bg);
            padding: 8px 12px;
            border-radius: 6px;
            font-size: 0.85rem;
            font-weight: 600;
            white-space: nowrap;
            z-index: 10000;
            pointer-events: none;
            box-shadow: 0 4px 12px var(--shared-border);
            opacity: 0;
            transition: opacity 0.2s ease;
        `;
    return tooltip;
  }

  /**
   * @param {any} tooltip
   * @param {any} mouseEvent
   * @param {any} percentage
   * @param {any} session
   */
  updateDragTooltip(tooltip, mouseEvent, percentage, session) {
    // Calculate time from percentage
    const timelineStartMinutes = 0; // 00:00 (midnight)
    const timelineRangeMinutes = 24 * 60; // 24 hours (full day)

    const startMinutes = timelineStartMinutes + (percentage / 100) * timelineRangeMinutes;
    const endMinutes = startMinutes + (session.duration || 25); // Default 25 min if no duration

    const startHour = Math.floor(startMinutes / 60);
    const startMin = Math.round(startMinutes % 60);
    const endHour = Math.floor(endMinutes / 60);
    const endMin = Math.round(endMinutes % 60);

    const startTime = `${startHour.toString().padStart(2, "0")}:${startMin.toString().padStart(2, "0")}`;
    const endTime = `${endHour.toString().padStart(2, "0")}:${endMin.toString().padStart(2, "0")}`;

    tooltip.textContent = `${startTime} - ${endTime}`;

    // Position tooltip near mouse
    tooltip.style.left = `${mouseEvent.clientX + 15}px`;
    tooltip.style.top = `${mouseEvent.clientY - 35}px`;
    tooltip.style.opacity = "1";
  }

  /**
   * @param {any} tooltip
   * @param {any} mouseEvent
   * @param {any} sessionElement
   */
  updateResizeTooltip(tooltip, mouseEvent, sessionElement) {
    const leftPercent = parseFloat(sessionElement.style.left);
    const widthPercent = parseFloat(sessionElement.style.width);
    const rightPercent = leftPercent + widthPercent;

    // Convert percentages to time (00:00 to 23:59 range)
    const timelineStartMinutes = 0; // 00:00 (midnight)
    const timelineRangeMinutes = 24 * 60; // 24 hours

    const startMinutes = timelineStartMinutes + (leftPercent / 100) * timelineRangeMinutes;
    const endMinutes = timelineStartMinutes + (rightPercent / 100) * timelineRangeMinutes;
    const durationMinutes = endMinutes - startMinutes;

    const startHour = Math.floor(startMinutes / 60);
    const startMin = Math.round(startMinutes % 60);
    const endHour = Math.floor(endMinutes / 60);
    const endMin = Math.round(endMinutes % 60);

    const startTime = `${startHour.toString().padStart(2, "0")}:${startMin.toString().padStart(2, "0")}`;
    const endTime = `${endHour.toString().padStart(2, "0")}:${endMin.toString().padStart(2, "0")}`;
    const duration = `${Math.round(durationMinutes)}min`;

    tooltip.textContent = `${startTime} - ${endTime} (${duration})`;

    // Position tooltip near mouse
    tooltip.style.left = `${mouseEvent.clientX + 15}px`;
    tooltip.style.top = `${mouseEvent.clientY - 35}px`;
    tooltip.style.opacity = "1";
  }

  /**
   * @param {any} session
   * @param {any} allSessions
   */
  calculateSessionOffset(session, allSessions) {
    if (!allSessions || allSessions.length <= 1) {
      return 0;
    }

    // Find the index of this session in the array
    const sessionIndex = allSessions.findIndex((/** @type {any} */ s) => s.id === session.id);

    // Each session gets its own row
    const rowHeight = 20; // 15px session height + 5px spacing
    return sessionIndex * rowHeight;
  }

  /** @param {any} session */
  createSessionHoverTooltip(session) {
    const tooltip = document.createElement("div");
    tooltip.className = "session-hover-tooltip";

    const content = document.createElement("div");
    content.className = "tooltip-content";

    const typeEl = document.createElement("div");
    typeEl.className = "tooltip-type";
    typeEl.textContent = "Focus Session";

    const timeEl = document.createElement("div");
    timeEl.className = "tooltip-time";
    timeEl.textContent = `${session.start_time} - ${session.end_time}`;

    const durationEl = document.createElement("div");
    durationEl.className = "tooltip-duration";
    durationEl.textContent = `${session.duration} minutes`;

    content.appendChild(typeEl);
    content.appendChild(timeEl);
    content.appendChild(durationEl);
    tooltip.appendChild(content);

    tooltip.style.position = "fixed";
    tooltip.style.zIndex = "1000";
    tooltip.style.opacity = "0";
    tooltip.style.transition = "opacity 0.2s ease";
    tooltip.style.pointerEvents = "none";

    return tooltip;
  }

  /**
   * @param {any} tooltip
   * @param {any} mouseEvent
   */
  updateHoverTooltip(tooltip, mouseEvent) {
    tooltip.style.left = `${mouseEvent.clientX + 10}px`;
    tooltip.style.top = `${mouseEvent.clientY - 10}px`;
    tooltip.style.opacity = "1";
  }

  async initSessionsTable(date = this.currentDate) {
    await this.populateSessionsTableForDate(date || this.currentDate);
    this.setupSessionsTableEventListeners();
  }

  setupSessionsTableEventListeners() {
    const exportBtn = document.getElementById("export-sessions-btn");
    if (!this._handleExportSessionsClick) {
      this._handleExportSessionsClick = () => {
        this.exportSessionsToExcel();
      };
    }
    if (exportBtn) {
      exportBtn.removeEventListener("click", this._handleExportSessionsClick);
      exportBtn.addEventListener("click", this._handleExportSessionsClick);
    }
  }

  getAllSessionsFromManager() {
    const allSessions = [];

    for (const [_dateString, sessions] of Object.entries(window.sessionManager.sessions)) {
      allSessions.push(...sessions);
    }

    return allSessions;
  }

  /** @param {any} date */
  async populateSessionsTableForDate(date) {
    const tableBody = document.getElementById("sessions-table-body");
    if (!tableBody || !window.sessionManager) {
      return;
    }

    const sessions = window.sessionManager.getSessionsForDate(date);
    tableBody.innerHTML = "";

    if (sessions.length === 0) {
      tableBody.innerHTML = `
                <tr>
                    <td colspan="5" class="sessions-table-empty">
                        No sessions found for selected date
                    </td>
                </tr>
            `;
      return;
    }

    // Sort sessions by time (newest first)
    sessions.sort((/** @type {any} */ a, /** @type {any} */ b) =>
      b.start_time.localeCompare(a.start_time)
    );

    for (const session of sessions) {
      const row = await this.createSessionTableRow(session);
      tableBody.appendChild(row);
    }
  }

  /** @param {any} session */
  async createSessionTableRow(session) {
    const row = document.createElement("tr");

    const sessionDate = new Date(session.created_at);
    const formattedDate = sessionDate.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    });

    const timeRange = `${session.start_time} - ${session.end_time}`;
    const tags = await this.getSessionTags(session.id);

    // Build tags cell using DOM to avoid XSS from tag names
    const tagsCell = document.createElement("td");
    const tagsDiv = document.createElement("div");
    tagsDiv.className = "session-tags";
    if (tags.length === 0) {
      const muted = document.createElement("span");
      muted.className = "text-muted";
      muted.textContent = "-";
      tagsDiv.appendChild(muted);
    } else if (tags.length === 1) {
      const tagSpan = document.createElement("span");
      tagSpan.className = "session-tag";
      tagSpan.textContent = tags[0].name;
      tagsDiv.appendChild(tagSpan);
    } else {
      const firstTag = tags[0];
      const remainingCount = tags.length - 1;
      const allTagNames = tags.map((/** @type {any} */ tag) => tag.name).join(", ");
      const compact = document.createElement("div");
      compact.className = "session-tags-compact";
      compact.title = allTagNames;
      const firstSpan = document.createElement("span");
      firstSpan.className = "session-tag";
      firstSpan.textContent = firstTag.name;
      const countSpan = document.createElement("span");
      countSpan.className = "session-tag-count";
      countSpan.textContent = `+${remainingCount}`;
      compact.append(firstSpan, countSpan);
      tagsDiv.appendChild(compact);
    }
    tagsCell.appendChild(tagsDiv);

    const dateTd = document.createElement("td");
    dateTd.textContent = formattedDate;
    const timeTd = document.createElement("td");
    timeTd.textContent = timeRange;
    const durationTd = document.createElement("td");
    durationTd.textContent = `${session.duration}m`;
    row.append(dateTd, timeTd, durationTd, tagsCell);

    // Build action buttons with addEventListener instead of inline onclick
    const actionsCell = document.createElement("td");
    const actionsDiv = document.createElement("div");
    actionsDiv.className = "session-actions";

    const editBtn = document.createElement("button");
    editBtn.type = "button";
    editBtn.className = "session-action-btn edit";
    editBtn.title = "Edit Session";
    editBtn.setAttribute("aria-label", "Edit session");
    editBtn.innerHTML = '<i class="ri-edit-line"></i>';
    editBtn.addEventListener("click", () => this.editSessionFromTable(session.id));

    const deleteBtn = document.createElement("button");
    deleteBtn.type = "button";
    deleteBtn.className = "session-action-btn delete";
    deleteBtn.title = "Delete Session";
    deleteBtn.setAttribute("aria-label", "Delete session");
    deleteBtn.innerHTML = '<i class="ri-delete-bin-line"></i>';
    deleteBtn.addEventListener("click", () => this.deleteSessionFromTable(session.id));

    actionsDiv.append(editBtn, deleteBtn);
    actionsCell.appendChild(actionsDiv);
    row.appendChild(actionsCell);

    return row;
  }

  /** @param {any} sessionId */
  async getSessionTags(sessionId) {
    // Get tags directly from session data
    if (window.sessionManager) {
      try {
        // Find the session in all dates
        for (const dateString in window.sessionManager.sessions) {
          const dateSessions = window.sessionManager.sessions[dateString];
          if (dateSessions) {
            const session = dateSessions.find((/** @type {any} */ s) => s.id === sessionId);
            if (session && session.tags) {
              return session.tags;
            }
          }
        }
      } catch (_error) {
        logger.debug("Tags not available for session:", sessionId);
      }
    }
    return [];
  }

  /** @param {any} sessionId */
  async deleteSessionFromTable(sessionId) {
    if (!window.sessionManager || !sessionId) {
      return;
    }

    try {
      let sessionFound = false;

      let deletedFromDate = null;
      for (const [dateString, sessions] of Object.entries(window.sessionManager.sessions)) {
        const sessionIndex = sessions.findIndex((/** @type {any} */ s) => s.id === sessionId);
        if (sessionIndex !== -1) {
          sessions.splice(sessionIndex, 1);
          sessionFound = true;
          deletedFromDate = dateString;
          logger.info("Session deleted successfully:", sessionId);
          break;
        }
      }

      if (!sessionFound) {
        logger.warn("Session not found for deletion:", sessionId);
        return;
      }

      await window.sessionManager.saveSessionsToStorage();

      window.dispatchEvent(
        new CustomEvent("sessionDeleted", {
          detail: { sessionId, date: deletedFromDate },
        })
      );

      const currentDate = this.selectedDate || this.currentDate;
      await this.populateSessionsTableForDate(currentDate);

      // Refresh other views (with error handling for each)
      try {
        await this.updateDailyChart();
      } catch (e) {
        logger.warn("Failed to update daily chart after deletion:", e);
      }

      try {
        await this.updateFocusSummary();
      } catch (e) {
        logger.warn("Failed to update focus summary after deletion:", e);
      }

      try {
        await this.updateWeeklySessionsChart();
      } catch (e) {
        logger.warn("Failed to update weekly chart after deletion:", e);
      }

      try {
        await this.updateTagUsageChart();
      } catch (e) {
        logger.warn("Failed to update tag usage chart after deletion:", e);
      }
    } catch (error) {
      logger.error("Error deleting session:", error);
      alert("Failed to delete session. Please try again.");
    }
  }

  /** @param {any} sessionId */
  async editSessionFromTable(sessionId) {
    if (!window.sessionManager || !sessionId) {
      return;
    }

    try {
      let sessionToEdit = null;
      let sessionDate = null;

      for (const [dateString, sessions] of Object.entries(window.sessionManager.sessions)) {
        const session = sessions.find((/** @type {any} */ s) => s.id === sessionId);
        if (session) {
          sessionToEdit = session;
          sessionDate = dateString;
          break;
        }
      }

      if (!sessionToEdit) {
        logger.warn("Session not found for editing:", sessionId);
        return;
      }

      window.sessionManager.openEditSessionModal(sessionToEdit, sessionDate);
    } catch (error) {
      logger.error("Error opening edit session modal:", error);
      alert("Failed to open edit session. Please try again.");
    }
  }

  async exportSessionsToExcel() {
    try {
      const currentDate = this.selectedDate || this.currentDate;
      const sessions = window.sessionManager.getSessionsForDate(currentDate);

      if (sessions.length === 0) {
        alert("No sessions to export for the selected period.");
        return;
      }

      const XLSX = window.XLSX;
      if (!XLSX) {
        logger.error("XLSX library not found");
        alert("Excel export functionality is not available.");
        return;
      }

      const exportData = [];
      for (const session of sessions) {
        const sessionDate = new Date(session.created_at);
        const formattedDate = sessionDate.toLocaleDateString("en-US", {
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
        });

        const tags = await this.getSessionTags(session.id);
        const tagNames = tags.map((/** @type {any} */ tag) => tag.name).join(", ");

        exportData.push({
          Date: formattedDate,
          "Start Time": session.start_time,
          "End Time": session.end_time,
          "Duration (minutes)": session.duration,
          Tags: tagNames || "-",
        });
      }

      exportData.sort((a, b) => {
        const dateComparison = new Date(b.Date).getTime() - new Date(a.Date).getTime();
        if (dateComparison !== 0) {
          return dateComparison;
        }
        return b["Start Time"].localeCompare(a["Start Time"]);
      });

      const ws = XLSX.utils.json_to_sheet(exportData);
      const wb = XLSX.utils.book_new();
      XLSX.utils.book_append_sheet(wb, ws, "Session History");

      const dateStr = currentDate.toISOString().split("T")[0];
      const defaultFilename = `presto-session-history-${dateStr}.xlsx`;

      if (window.__TAURI__) {
        try {
          const tauriDialog = window.__TAURI__.dialog;
          const tauriCore = window.__TAURI__.core;
          if (!tauriDialog || !tauriCore) {
            throw new Error("Tauri APIs not available");
          }

          const filePath = await tauriDialog.save({
            defaultPath: defaultFilename,
            filters: [
              {
                name: "Excel files",
                extensions: ["xlsx"],
              },
            ],
          });

          if (filePath) {
            const wbout = XLSX.write(wb, { bookType: "xlsx", type: "base64" });

            await tauriCore.invoke("write_excel_file", {
              path: filePath,
              data: wbout,
            });

            logger.info(`Exported ${sessions.length} sessions to ${filePath}`);
            alert(`Sessions exported successfully to:\n${filePath}`);
          } else {
            logger.info("Export cancelled by user");
          }
        } catch (tauriError) {
          logger.error("Tauri save error:", tauriError);
          XLSX.writeFile(wb, defaultFilename);
          logger.warn(`Tauri save failed, using fallback download: ${defaultFilename}`);
          alert(`File saved to Downloads folder as: ${defaultFilename}`);
        }
      } else {
        XLSX.writeFile(wb, defaultFilename);
        logger.info(`Exported ${sessions.length} sessions to ${defaultFilename}`);
      }
    } catch (error) {
      logger.error("Error exporting sessions:", error);
      alert("Failed to export sessions. Please try again.");
    }
  }
}
