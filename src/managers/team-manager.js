// Team Manager for Team Dashboard
import { logger } from "../utils/logger.js";

export class TeamManager {
  constructor() {
    this.teams = [];
    this.initialized = false;
  }

  async init() {
    if (this.initialized) {
      logger.debug("TeamManager already initialized, skipping...");
      return;
    }

    this.initialized = true;
    logger.info("Initializing TeamManager...");

    // Initialize with demo data
    this.initializeDemoData();
    this.renderTeams();
    this.updateTeamStats();

    // Update team data every 30 seconds to simulate real-time updates
    setInterval(() => {
      this.updateDemoData();
      this.renderTeams();
      this.updateTeamStats();
    }, 30000);
  }

  initializeDemoData() {
    this.teams = [
      {
        id: 1,
        name: "Team Frontend",
        description: "React & Vue.js Development",
        members: [
          {
            id: 1,
            name: "Marco Rossi",
            role: "Frontend Developer",
            avatar: "MR",
            status: "focus",
            timer: "18:35",
            activity: "Working on React components",
            lastSeen: new Date(),
            totalFocusToday: 180,
            currentSessionStart: new Date(Date.now() - 6.5 * 60 * 1000),
          },
          {
            id: 2,
            name: "Sara Bianchi",
            role: "UX Designer",
            avatar: "SB",
            status: "break",
            timer: "3:20",
            activity: "Short break",
            lastSeen: new Date(),
            totalFocusToday: 125,
            currentSessionStart: new Date(Date.now() - 1.5 * 60 * 1000),
          },
          {
            id: 3,
            name: "Francesco Galli",
            role: "Frontend Developer",
            avatar: "FG",
            status: "focus",
            timer: "22:10",
            activity: "Code review session",
            lastSeen: new Date(),
            totalFocusToday: 165,
            currentSessionStart: new Date(Date.now() - 2.8 * 60 * 1000),
          },
          {
            id: 4,
            name: "Elena Conti",
            role: "UI Designer",
            avatar: "EC",
            status: "offline",
            timer: "--:--",
            activity: "Last seen yesterday",
            lastSeen: new Date(Date.now() - 24 * 60 * 60 * 1000),
            totalFocusToday: 0,
            currentSessionStart: null,
          },
        ],
      },
      {
        id: 2,
        name: "Team Backend",
        description: "API & Database Development",
        members: [
          {
            id: 5,
            name: "Luca Verdi",
            role: "Backend Developer",
            avatar: "LV",
            status: "focus",
            timer: "12:45",
            activity: "API development",
            lastSeen: new Date(),
            totalFocusToday: 205,
            currentSessionStart: new Date(Date.now() - 12.3 * 60 * 1000),
          },
          {
            id: 6,
            name: "Giulia Neri",
            role: "Product Manager",
            avatar: "GN",
            status: "long-break",
            timer: "15:00",
            activity: "Long break",
            lastSeen: new Date(),
            totalFocusToday: 150,
            currentSessionStart: new Date(Date.now() - 5 * 60 * 1000),
          },
          {
            id: 7,
            name: "Andrea Ferrari",
            role: "DevOps Engineer",
            avatar: "AF",
            status: "privacy",
            timer: "--:--",
            activity: "Privacy mode enabled",
            lastSeen: new Date(Date.now() - 45 * 60 * 1000),
            totalFocusToday: 95,
            currentSessionStart: null,
          },
          {
            id: 8,
            name: "Chiara Romano",
            role: "QA Engineer",
            avatar: "CR",
            status: "offline",
            timer: "--:--",
            activity: "Last seen 2 hours ago",
            lastSeen: new Date(Date.now() - 2 * 60 * 60 * 1000),
            totalFocusToday: 75,
            currentSessionStart: null,
          },
        ],
      },
    ];
  }

  updateDemoData() {
    // Simulate status changes for demo
    this.teams.forEach((team) => {
      team.members.forEach((member) => {
        if (
          member.status === "focus" ||
          member.status === "break" ||
          member.status === "long-break"
        ) {
          // Update timer
          if (member.currentSessionStart) {
            const elapsed = Math.floor((Date.now() - member.currentSessionStart.getTime()) / 1000);
            const _minutes = Math.floor(elapsed / 60);
            const _seconds = elapsed % 60;

            if (member.status === "focus") {
              // Focus sessions are 25 minutes, count down
              const remaining = 25 * 60 - elapsed;
              if (remaining > 0) {
                const remMin = Math.floor(remaining / 60);
                const remSec = remaining % 60;
                member.timer = `${remMin.toString().padStart(2, "0")}:${remSec.toString().padStart(2, "0")}`;
              } else {
                // Session ended, randomly change to break or continue
                if (Math.random() > 0.7) {
                  member.status = "break";
                  member.currentSessionStart = new Date();
                  member.timer = "05:00";
                  member.activity = "Short break";
                }
              }
            } else if (member.status === "break") {
              // Break sessions are 5 minutes, count down
              const remaining = 5 * 60 - elapsed;
              if (remaining > 0) {
                const remMin = Math.floor(remaining / 60);
                const remSec = remaining % 60;
                member.timer = `${remMin.toString().padStart(2, "0")}:${remSec.toString().padStart(2, "0")}`;
              } else {
                // Break ended, randomly start focus or stay on break
                if (Math.random() > 0.5) {
                  member.status = "focus";
                  member.currentSessionStart = new Date();
                  member.timer = "25:00";
                  member.activity = this.getRandomFocusActivity();
                }
              }
            } else if (member.status === "long-break") {
              // Long break sessions are 15-20 minutes
              const remaining = 20 * 60 - elapsed;
              if (remaining > 0) {
                const remMin = Math.floor(remaining / 60);
                const remSec = remaining % 60;
                member.timer = `${remMin.toString().padStart(2, "0")}:${remSec.toString().padStart(2, "0")}`;
              }
            }
          }

          // Update last seen
          member.lastSeen = new Date();
        }

        // Randomly change some statuses
        if (Math.random() > 0.95) {
          const possibleStatuses = ["focus", "break", "privacy"];
          const newStatus = possibleStatuses[Math.floor(Math.random() * possibleStatuses.length)];
          if (newStatus !== member.status && member.status !== "offline") {
            member.status = newStatus;
            member.currentSessionStart = new Date();
            if (newStatus === "focus") {
              member.timer = "25:00";
              member.activity = this.getRandomFocusActivity();
            } else if (newStatus === "break") {
              member.timer = "05:00";
              member.activity = "Short break";
            } else if (newStatus === "privacy") {
              member.timer = "--:--";
              member.activity = "Privacy mode enabled";
              member.currentSessionStart = null;
            }
          }
        }
      });
    });
  }

  getRandomFocusActivity() {
    const activities = [
      "Working on React components",
      "API development",
      "Code review session",
      "Writing documentation",
      "Bug fixing",
      "Database optimization",
      "UI/UX design",
      "Testing new features",
      "Refactoring code",
      "Planning sprint tasks",
    ];
    return activities[Math.floor(Math.random() * activities.length)];
  }

  renderTeams() {
    const container = document.getElementById("team-members-grid");
    if (!container) {
      return;
    }

    container.innerHTML = "";

    this.teams.forEach((team) => {
      // Sort members by status priority: active (focus) > break/long-break > offline
      const sortedMembers = this.sortMembersByStatus(team.members);

      // Create team section
      const teamSection = this.createTeamSection(team, sortedMembers);
      container.appendChild(teamSection);
    });
  }

  sortMembersByStatus(members) {
    return [...members].sort((a, b) => {
      const getPriority = (status) => {
        switch (status) {
          case "focus":
            return 0;
          case "break":
          case "long-break":
            return 1;
          case "privacy":
            return 1;
          case "offline":
            return 2;
          default:
            return 3;
        }
      };
      return getPriority(a.status) - getPriority(b.status);
    });
  }

  createTeamSection(team, members) {
    const section = document.createElement("div");
    section.className = "team-section base-card-compact";

    const header = document.createElement("div");
    header.className = "team-header";
    header.innerHTML = `
            <h3>${team.name}</h3>
            <p class="team-description">${team.description}</p>
        `;

    const membersTable = document.createElement("div");
    membersTable.className = "team-members-table";

    members.forEach((member) => {
      const memberRow = this.createMemberRow(member);
      membersTable.appendChild(memberRow);
    });

    section.appendChild(header);
    section.appendChild(membersTable);

    return section;
  }

  createMemberRow(member) {
    const row = document.createElement("div");
    row.className = `member-row row-base row-three-col status-${member.status}`;

    const statusInfo = this.getStatusInfo(member.status);
    const onlineStatus = this.getOnlineStatus(member);

    row.innerHTML = `
            <div class="member-basic-info">
                <div class="member-avatar-small">
                    ${member.avatar}
                    <div class="online-indicator-small ${onlineStatus}"></div>
                </div>
                <div class="member-details">
                    <span class="member-name text-ellipsis">${member.name}</span>
                    <span class="member-role-small text-ellipsis">${member.role}</span>
                </div>
            </div>
            
            <div class="member-status-info flex-center">
                <div class="status-badge-small badge-base badge-${member.status}">
                    <i class="status-icon ${statusInfo.icon}"></i>
                    <span>${statusInfo.label}</span>
                </div>
                <div class="member-timer-small ${member.status}">
                    ${member.timer}
                </div>
            </div>
            
            <div class="member-activity-small text-ellipsis">
                ${member.activity}
            </div>
        `;

    return row;
  }

  createMemberCard(member) {
    const card = document.createElement("div");
    card.className = `team-member-card status-${member.status}`;

    const statusInfo = this.getStatusInfo(member.status);
    const onlineStatus = this.getOnlineStatus(member);

    card.innerHTML = `
            <div class="member-header">
                <div class="member-avatar">
                    ${member.avatar}
                    <div class="online-indicator ${onlineStatus}"></div>
                </div>
                <div class="member-info">
                    <h3>${member.name}</h3>
                    <p class="member-role">${member.role}</p>
                </div>
            </div>
            
            <div class="status-badge ${member.status}">
                <i class="status-icon ${statusInfo.icon}"></i>
                <span>${statusInfo.label}</span>
            </div>
            
            <div class="member-timer ${member.status}">
                ${member.timer}
            </div>
            
            <div class="member-activity">
                ${member.activity}
            </div>
        `;

    return card;
  }

  getStatusInfo(status) {
    const statusMap = {
      focus: { icon: "ri-brain-line", label: "Deep Focus" },
      break: { icon: "ri-cup-line", label: "Short Break" },
      "long-break": { icon: "ri-moon-line", label: "Long Break" },
      offline: { icon: "ri-checkbox-blank-circle-fill", label: "Offline" },
      privacy: { icon: "ri-lock-line", label: "Privacy Mode" },
    };
    return statusMap[status] || { icon: "ri-question-line", label: "Unknown" };
  }

  getOnlineStatus(member) {
    const now = new Date();
    const lastSeenMinutes = (now - member.lastSeen) / (1000 * 60);

    if (member.status === "offline") {
      return "offline";
    }
    if (member.status === "privacy") {
      return "privacy";
    }
    if (lastSeenMinutes < 5) {
      return "online";
    }
    return "offline";
  }

  updateTeamStats() {
    const stats = this.calculateTeamStats();

    document.getElementById("team-focusing").textContent = stats.focusing;
    document.getElementById("team-on-break").textContent = stats.onBreak;
    document.getElementById("team-privacy").textContent = stats.privacy;
    document.getElementById("team-offline").textContent = stats.offline;
  }

  calculateTeamStats() {
    const stats = {
      focusing: 0,
      onBreak: 0,
      privacy: 0,
      offline: 0,
    };

    this.teams.forEach((team) => {
      team.members.forEach((member) => {
        switch (member.status) {
          case "focus":
            stats.focusing++;
            break;
          case "break":
          case "long-break":
            stats.onBreak++;
            break;
          case "privacy":
            stats.privacy++;
            break;
          case "offline":
            stats.offline++;
            break;
        }
      });
    });

    return stats;
  }
}
