// Tag Statistics Utility for generating pie chart data and visualization
import { logger } from "./logger.js";

export class TagStatistics {
  constructor() {
    this.tagColors = [
      "#3b82f6", // Blue
      "#10b981", // Green
      "#f59e0b", // Yellow
      "#ef4444", // Red
      "#8b5cf6", // Purple
      "#06b6d4", // Cyan
      "#f97316", // Orange
      "#84cc16", // Lime
      "#ec4899", // Pink
      "#6b7280", // Gray
      "#14b8a6", // Teal
      "#a855f7", // Violet
      "#eab308", // Amber
      "#22c55e", // Emerald
      "#0ea5e9", // Sky
    ];
  }

  /**
   * Get tag usage statistics for a specific time period
   * @param {Array} sessions - Array of session objects
   * @param {Array} tags - Array of available tags
   * @param {Date} startDate - Start date for filtering
   * @param {Date} endDate - End date for filtering
   * @returns {Object} Tag statistics with percentages and durations
   */
  getTagUsageStatistics(sessions, tags, startDate, endDate) {
    // Filter sessions within the date range
    const filteredSessions = sessions.filter((session) => {
      const sessionDate = new Date(session.date || session.created_at);
      return sessionDate >= startDate && sessionDate <= endDate;
    });

    // Calculate tag usage from session data
    const tagUsage = new Map();
    let totalDuration = 0;

    // Process sessions to calculate tag durations
    filteredSessions.forEach((session) => {
      // All sessions are focus sessions now
      if (session.duration > 0) {
        // Handle various formats for tags field (backward compatibility)
        let sessionTags = [];
        if (Array.isArray(session.tags)) {
          sessionTags = session.tags;
        } else if (session.tags) {
          // In case tags is not an array but exists
          sessionTags = [session.tags];
        }
        // sessionTags will be empty array if session.tags is null/undefined

        if (sessionTags.length === 0) {
          // Session without tags - assign to "Untagged"
          const untaggedKey = "untagged";
          const current = tagUsage.get(untaggedKey) || { duration: 0, sessions: 0 };
          tagUsage.set(untaggedKey, {
            duration: current.duration + session.duration,
            sessions: current.sessions + 1,
            tag: { id: "untagged", name: "Untagged", icon: "ri-price-tag-line", color: "#6b7280" },
          });
          totalDuration += session.duration;
        } else {
          // Distribute session duration equally among all tags
          const durationPerTag = session.duration / sessionTags.length;
          sessionTags.forEach((sessionTag) => {
            // Handle both tag objects and tag IDs
            const tagId = typeof sessionTag === "string" ? sessionTag : sessionTag.id;
            const tag =
              typeof sessionTag === "object" ? sessionTag : tags.find((t) => t.id === tagId);

            if (tag) {
              const current = tagUsage.get(tagId) || { duration: 0, sessions: 0 };
              tagUsage.set(tagId, {
                duration: current.duration + durationPerTag,
                sessions: current.sessions + 1,
                tag,
              });
            }
          });
          totalDuration += session.duration;
        }
      }
    });

    // Convert to array and calculate percentages
    const tagStats = Array.from(tagUsage.entries()).map(([tagId, data], index) => ({
      tagId,
      tag: data.tag,
      duration: Math.round(data.duration * 60), // Convert to seconds
      sessions: data.sessions,
      percentage: totalDuration > 0 ? (data.duration / totalDuration) * 100 : 0,
      color: this.tagColors[index % this.tagColors.length],
    }));

    // Sort by duration (highest first)
    tagStats.sort((a, b) => b.duration - a.duration);

    return {
      stats: tagStats,
      totalDuration: Math.round(totalDuration * 60), // Convert to seconds
      totalSessions: filteredSessions.length, // All sessions are focus sessions now
    };
  }

  /**
   * Get current week tag statistics
   * @param {Array} sessions - Array of session objects
   * @param {Array} tags - Array of available tags
   * @returns {Object} Tag statistics for current week
   */
  getCurrentWeekTagStats(sessions, tags) {
    const now = new Date();
    const startOfWeek = new Date(now);
    startOfWeek.setDate(now.getDate() - now.getDay()); // Start of week (Sunday)
    startOfWeek.setHours(0, 0, 0, 0);

    const endOfWeek = new Date(startOfWeek);
    endOfWeek.setDate(startOfWeek.getDate() + 6); // End of week (Saturday)
    endOfWeek.setHours(23, 59, 59, 999);

    return this.getTagUsageStatistics(sessions, tags, startOfWeek, endOfWeek);
  }

  /**
   * Format duration in seconds to human readable format
   * @param {number} seconds - Duration in seconds
   * @returns {string} Formatted duration (e.g., "2h 30m", "45m", "30s")
   */
  formatDuration(seconds) {
    if (seconds < 60) {
      return `${seconds}s`;
    }

    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) {
      return `${minutes}m`;
    }

    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;

    if (remainingMinutes === 0) {
      return `${hours}h`;
    }
    return `${hours}h ${remainingMinutes}m`;
  }

  /**
   * Generate CSS conic-gradient for pie chart
   * @param {Array} tagStats - Array of tag statistics
   * @returns {string} CSS conic-gradient string
   */
  generatePieChartGradient(tagStats) {
    if (!tagStats || tagStats.length === 0) {
      return "conic-gradient(#e5e7eb 0deg 360deg)";
    }

    let currentAngle = 0;
    const gradientStops = [];

    tagStats.forEach((stat, _index) => {
      const angle = (stat.percentage / 100) * 360;
      const nextAngle = currentAngle + angle;

      gradientStops.push(`${stat.color} ${currentAngle}deg ${nextAngle}deg`);

      currentAngle = nextAngle;
    });

    return `conic-gradient(${gradientStops.join(", ")})`;
  }

  /**
   * Render pie chart and legend
   * @param {string} chartContainerId - ID of the chart container element
   * @param {string} legendContainerId - ID of the legend container element
   * @param {Object} tagStatsData - Tag statistics data
   */
  renderTagPieChart(chartContainerId, legendContainerId, tagStatsData) {
    const chartContainer = document.getElementById(chartContainerId);
    const legendContainer = document.getElementById(legendContainerId);

    if (!chartContainer || !legendContainer) {
      logger.error("Tag pie chart containers not found");
      return;
    }

    const { stats, totalDuration } = tagStatsData;

    // Clear containers
    chartContainer.innerHTML = "";
    legendContainer.innerHTML = "";

    if (!stats || stats.length === 0 || totalDuration === 0) {
      // Show placeholder
      chartContainer.innerHTML = `
                <div class="pie-chart-placeholder">
                    <i class="ri-pie-chart-line"></i>
                    <span>No data available</span>
                </div>
            `;
      return;
    }

    // Create pie chart with CSS conic-gradient
    const gradient = this.generatePieChartGradient(stats);
    chartContainer.style.background = gradient;
    chartContainer.style.border = "1px solid #e5e7eb";

    // Show only top 5 tags in legend for better space utilization
    const topStats = stats.slice(0, 5);
    const remainingStats = stats.slice(5);

    // Create legend for top tags using safe DOM construction
    topStats.forEach((stat) => {
      const legendItem = document.createElement("div");
      legendItem.className = "tag-legend-item";

      const colorDiv = document.createElement("div");
      colorDiv.className = "tag-legend-color";
      colorDiv.style.backgroundColor = stat.color;

      const infoDiv = document.createElement("div");
      infoDiv.className = "tag-legend-info";

      const nameDiv = document.createElement("div");
      nameDiv.className = "tag-legend-name";
      if (stat.tag.icon.startsWith("ri-")) {
        const icon = document.createElement("i");
        icon.className = stat.tag.icon;
        nameDiv.appendChild(icon);
        nameDiv.append(` ${stat.tag.name}`);
      } else {
        nameDiv.textContent = `${stat.tag.icon} ${stat.tag.name}`;
      }

      const statsDiv = document.createElement("div");
      statsDiv.className = "tag-legend-stats";

      const timeSpan = document.createElement("span");
      timeSpan.className = "tag-legend-time";
      timeSpan.textContent = this.formatDuration(stat.duration);

      const percentSpan = document.createElement("span");
      percentSpan.className = "tag-legend-percent";
      percentSpan.textContent = `${stat.percentage.toFixed(1)}%`;

      statsDiv.append(timeSpan, percentSpan);
      infoDiv.append(nameDiv, statsDiv);
      legendItem.append(colorDiv, infoDiv);
      legendContainer.appendChild(legendItem);
    });

    // If there are more tags, show a summary
    if (remainingStats.length > 0) {
      const remainingDuration = remainingStats.reduce((sum, stat) => sum + stat.duration, 0);
      const remainingPercentage = remainingStats.reduce((sum, stat) => sum + stat.percentage, 0);

      const othersItem = document.createElement("div");
      othersItem.className = "tag-legend-item tag-legend-others";

      othersItem.innerHTML = `
                <div class="tag-legend-color" style="background: linear-gradient(45deg, ${remainingStats
                  .slice(0, 3)
                  .map((s) => s.color)
                  .join(", ")})"></div>
                <div class="tag-legend-info">
                    <div class="tag-legend-name">
                        <i class="ri-more-line"></i> ${remainingStats.length} others
                    </div>
                    <div class="tag-legend-stats">
                        <span class="tag-legend-time">${this.formatDuration(remainingDuration)}</span>
                        <span class="tag-legend-percent">${remainingPercentage.toFixed(1)}%</span>
                    </div>
                </div>
            `;

      legendContainer.appendChild(othersItem);
    }
  }
}
