class TagManager {
  constructor() {
    this.tags = [];
    this.currentTags = []; // Tags currently selected for the session
    this.activeSessionTags = new Map(); // Map of tag_id -> start_time for tracking
    this.isDropdownOpen = false;
    this.selectedIcon = "ri-brain-line";

    this.initializeElements();
    this.bindEvents();
    this.loadTags();

    // Initialize icon selection
    this.resetIconSelection();
  }

  initializeElements() {
    this.timerStatus = document.getElementById("timer-status");
    this.statusText = document.getElementById("status-text");
    this.statusIcon = document.getElementById("status-icon");
    this.dropdownArrow = document.getElementById("tag-dropdown-arrow");
    this.dropdownMenu = document.getElementById("tag-dropdown-menu");
    this.tagList = document.getElementById("tag-list");
    this.newTagName = document.getElementById("new-tag-name");
    this.iconSelector = document.getElementById("icon-selector-dropdown");
    this.selectedIconBtn = document.getElementById("selected-icon-btn");
    this.selectedIconDisplay = document.getElementById("selected-icon-display");
    this.createTagBtn = document.getElementById("create-tag-btn");

    // Debug: check if all elements are found
    console.log("TagManager elements:", {
      timerStatus: !!this.timerStatus,
      dropdownMenu: !!this.dropdownMenu,
      tagList: !!this.tagList,
      newTagName: !!this.newTagName,
      iconSelector: !!this.iconSelector,
      createTagBtn: !!this.createTagBtn,
    });

    if (!this.dropdownMenu) {
      console.error("Dropdown menu not found!");
    }
    if (!this.newTagName) {
      console.error("New tag input not found!");
    }
  }

  bindEvents() {
    if (this.timerStatus && this.dropdownMenu) {
      this.timerStatus.addEventListener("click", () => {
        this.toggleDropdown();
      });

      document.addEventListener("click", (e) => {
        if (!this.timerStatus.contains(e.target) && !this.dropdownMenu.contains(e.target)) {
          this.closeDropdown();
        }
      });
    }

    if (this.selectedIconBtn && this.iconSelector) {
      this.selectedIconBtn.addEventListener("click", () => {
        this.toggleIconSelector();
      });

      this.iconSelector.addEventListener("click", (e) => {
        const iconOption = e.target.closest(".icon-option, .emoji-option");
        if (iconOption) {
          this.selectIcon(iconOption);
        }
      });

      document.addEventListener("click", (e) => {
        if (!this.selectedIconBtn.contains(e.target) && !this.iconSelector.contains(e.target)) {
          this.closeIconSelector();
        }
      });
    }

    if (this.createTagBtn && this.newTagName) {
      this.createTagBtn.addEventListener("click", () => {
        this.createNewTag();
      });

      this.newTagName.addEventListener("keypress", (e) => {
        if (e.key === "Enter") {
          this.createNewTag();
        }
      });

      this.newTagName.addEventListener("input", () => {
        this.updateCreateButtonState();
      });
    }
  }

  async loadTags() {
    try {
      // Check if Tauri is available
      if (
        typeof window.__TAURI__ === "undefined" ||
        typeof window.__TAURI__.invoke !== "function"
      ) {
        console.warn("Tauri is not available, using localStorage fallback");
        // Load from localStorage
        const savedTags = localStorage.getItem("presto-tags");
        if (savedTags) {
          try {
            this.tags = JSON.parse(savedTags);
          } catch (_parseError) {
            console.error("TagManager: corrupted tags in localStorage, resetting");
            localStorage.removeItem("presto-tags");
            this.tags = [];
          }
        }
        if (this.tags.length === 0) {
          this.tags = [
            {
              id: "default-focus",
              name: "Focus",
              icon: "ri-brain-line",
              color: "#4CAF50",
              created_at: new Date().toISOString(),
            },
          ];
          this.saveTagsToLocalStorage();
        }
        this.renderTagList();
        if (this.currentTags.length === 0) {
          this.currentTags = [this.tags[0]];
          this.updateStatusDisplay();
        }
        return;
      }

      this.tags = await window.__TAURI__.invoke("load_tags");
      this.renderTagList();

      // Set default tag if no current tags selected
      if (this.currentTags.length === 0 && this.tags.length > 0) {
        this.currentTags = [this.tags[0]];
        this.updateStatusDisplay();
      }
    } catch (error) {
      console.error("Failed to load tags:", error);
      // Fallback to localStorage or default tag
      const savedTags = localStorage.getItem("presto-tags");
      if (savedTags) {
        try {
          this.tags = JSON.parse(savedTags);
        } catch (_parseError) {
          console.error("TagManager: corrupted tags in localStorage, resetting");
          localStorage.removeItem("presto-tags");
          this.tags = [];
        }
      }
      if (this.tags.length === 0) {
        this.tags = [
          {
            id: "default-focus",
            name: "Focus",
            icon: "ri-brain-line",
            color: "#4CAF50",
            created_at: new Date().toISOString(),
          },
        ];
        this.saveTagsToLocalStorage();
      }
      this.renderTagList();
      if (this.currentTags.length === 0) {
        this.currentTags = [this.tags[0]];
        this.updateStatusDisplay();
      }
    }
  }

  renderTagList() {
    this.tagList.innerHTML = "";

    this.tags.forEach((tag) => {
      const tagItem = document.createElement("div");
      tagItem.className = "tag-item";
      tagItem.dataset.tagId = tag.id;

      // Check if tag is currently selected
      const isSelected = this.currentTags.some((t) => t.id === tag.id);
      if (isSelected) {
        tagItem.classList.add("selected");
      }

      const iconWrap = document.createElement("div");
      iconWrap.className = "tag-item-icon";
      if (typeof tag.icon === "string" && tag.icon.startsWith("ri-")) {
        const i = document.createElement("i");
        i.className = tag.icon;
        iconWrap.appendChild(i);
      } else {
        iconWrap.textContent = String(tag.icon ?? "");
      }

      const nameEl = document.createElement("div");
      nameEl.className = "tag-item-name";
      nameEl.textContent = tag.name;

      const deleteEl = document.createElement("div");
      deleteEl.className = "tag-item-delete ri-delete-bin-line";
      deleteEl.dataset.tagId = String(tag.id);

      tagItem.append(iconWrap, nameEl, deleteEl);

      // Tag selection event
      tagItem.addEventListener("click", (e) => {
        if (!e.target.classList.contains("tag-item-delete")) {
          e.stopPropagation(); // Prevent event bubbling
          this.toggleTag(tag);
        }
      });

      // Tag deletion event
      const deleteBtn = tagItem.querySelector(".tag-item-delete");
      deleteBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.deleteTag(tag.id);
      });

      this.tagList.appendChild(tagItem);
    });
  }

  toggleTag(tag) {
    const existingIndex = this.currentTags.findIndex((t) => t.id === tag.id);

    if (existingIndex !== -1) {
      // Remove tag and stop tracking
      this.currentTags.splice(existingIndex, 1);
      this.stopTagTracking(tag.id);
    } else {
      // Add tag and start tracking if timer is running
      this.currentTags.push(tag);
      if (window.pomodoroTimer && window.pomodoroTimer.isRunning) {
        this.startTagTracking(tag.id);
      }
    }

    this.updateStatusDisplay();
    this.renderTagList();
  }

  async createNewTag() {
    const name = this.newTagName.value.trim();
    if (!name) {
      return;
    }

    const newTag = {
      id: `tag-${Date.now()}`,
      name,
      icon: this.selectedIcon,
      color: "#4CAF50", // Default color
      created_at: new Date().toISOString(),
    };

    try {
      if (window.__TAURI__ && typeof window.__TAURI__.invoke === "function") {
        await window.__TAURI__.invoke("save_tag", newTag);
      } else {
        // Save to localStorage as fallback
        this.saveTagsToLocalStorage();
      }
      this.tags.push(newTag);

      // If using localStorage, save the updated tags
      if (
        typeof window.__TAURI__ === "undefined" ||
        typeof window.__TAURI__.invoke !== "function"
      ) {
        this.saveTagsToLocalStorage();
      }

      this.renderTagList();

      // Clear form
      this.newTagName.value = "";
      this.resetIconSelection();
      this.updateCreateButtonState();
    } catch (error) {
      console.error("Failed to create tag:", error);
    }
  }

  async deleteTag(tagId) {
    if (this.tags.length <= 1) {
      alert("Non puoi eliminare l'ultima tag.");
      return;
    }

    try {
      if (window.__TAURI__ && typeof window.__TAURI__.invoke === "function") {
        await window.__TAURI__.invoke("delete_tag", tagId);
      }

      // Remove from local arrays
      this.tags = this.tags.filter((t) => t.id !== tagId);
      this.currentTags = this.currentTags.filter((t) => t.id !== tagId);

      // If using localStorage, save the updated tags
      if (
        typeof window.__TAURI__ === "undefined" ||
        typeof window.__TAURI__.invoke !== "function"
      ) {
        this.saveTagsToLocalStorage();
      }

      // Stop tracking if active
      this.stopTagTracking(tagId);

      // If no tags selected, select the first available
      if (this.currentTags.length === 0 && this.tags.length > 0) {
        this.currentTags = [this.tags[0]];
      }

      this.updateStatusDisplay();
      this.renderTagList();
    } catch (error) {
      console.error("Failed to delete tag:", error);
    }
  }

  toggleIconSelector() {
    const isOpen = this.iconSelector.classList.contains("active");
    if (isOpen) {
      this.closeIconSelector();
    } else {
      this.openIconSelector();
    }
  }

  openIconSelector() {
    this.iconSelector.classList.add("active");
    this.selectedIconBtn.classList.add("active");
  }

  closeIconSelector() {
    this.iconSelector.classList.remove("active");
    this.selectedIconBtn.classList.remove("active");
  }

  selectIcon(iconOption) {
    // Remove previous selection
    this.iconSelector.querySelectorAll(".selected").forEach((el) => {
      el.classList.remove("selected");
    });

    // Select new icon
    iconOption.classList.add("selected");
    this.selectedIcon = iconOption.dataset.icon;

    // Update the display button
    this.updateSelectedIconDisplay();
    this.updateCreateButtonState();
  }

  updateSelectedIconDisplay() {
    const iconDisplay = this.selectedIconDisplay;
    if (this.selectedIcon.startsWith("ri-")) {
      iconDisplay.className = this.selectedIcon;
      iconDisplay.textContent = "";
      iconDisplay.style.fontFamily = "remixicon";
    } else {
      // For emoji
      iconDisplay.className = "";
      iconDisplay.textContent = this.selectedIcon;
      iconDisplay.style.fontFamily = "inherit";
    }
  }

  resetIconSelection() {
    this.iconSelector.querySelectorAll(".selected").forEach((el) => {
      el.classList.remove("selected");
    });

    // Select default icon
    const defaultIcon = this.iconSelector.querySelector('[data-icon="ri-brain-line"]');
    if (defaultIcon) {
      defaultIcon.classList.add("selected");
      this.selectedIcon = "ri-brain-line";
      this.updateSelectedIconDisplay();
    }
  }

  updateCreateButtonState() {
    const hasName = this.newTagName.value.trim().length > 0;
    this.createTagBtn.disabled = !hasName;
  }

  saveTagsToLocalStorage() {
    try {
      localStorage.setItem("presto-tags", JSON.stringify(this.tags));
    } catch (error) {
      console.error("Failed to save tags to localStorage:", error);
    }
  }

  toggleDropdown() {
    if (this.isDropdownOpen) {
      this.closeDropdown();
    } else {
      this.openDropdown();
    }
  }

  openDropdown() {
    console.log("Opening dropdown...");
    this.isDropdownOpen = true;
    this.timerStatus.classList.add("active");
    this.dropdownMenu.classList.add("active");
    console.log(
      "Dropdown classes added, menu visible:",
      this.dropdownMenu.classList.contains("active")
    );
    this.loadTags(); // Refresh tags when opening
  }

  closeDropdown() {
    this.isDropdownOpen = false;
    this.timerStatus.classList.remove("active");
    this.dropdownMenu.classList.remove("active");
  }

  updateStatusDisplay() {
    if (this.currentTags.length === 0) {
      this.statusText.textContent = "Focus";
      this.statusIcon.className = "ri-brain-line";
      this.statusIcon.style.fontFamily = "remixicon";
      this.statusIcon.textContent = "";
      return;
    }

    if (this.currentTags.length === 1) {
      const tag = this.currentTags[0];
      this.statusText.textContent = tag.name;

      if (tag.icon.startsWith("ri-")) {
        this.statusIcon.className = tag.icon;
        this.statusIcon.style.fontFamily = "remixicon";
        this.statusIcon.textContent = "";
      } else {
        // For emoji, we need to handle differently
        this.statusIcon.style.fontFamily = "inherit";
        this.statusIcon.textContent = tag.icon;
        this.statusIcon.className = "";
      }
    } else {
      // Multiple tags selected
      this.statusText.textContent = `${this.currentTags.length} Tags`;
      this.statusIcon.className = "ri-price-tag-3-line";
      this.statusIcon.style.fontFamily = "remixicon";
      this.statusIcon.textContent = "";
    }
  }

  // Tag tracking methods for time measurement
  startTagTracking(tagId) {
    if (!this.activeSessionTags.has(tagId)) {
      this.activeSessionTags.set(tagId, Date.now());
    }
  }

  stopTagTracking(tagId) {
    if (this.activeSessionTags.has(tagId)) {
      const startTime = this.activeSessionTags.get(tagId);
      const duration = Math.floor((Date.now() - startTime) / 1000); // Duration in seconds

      // Save session tag record
      this.saveSessionTag(tagId, duration);
      this.activeSessionTags.delete(tagId);
    }
  }

  async saveSessionTag(tagId, duration) {
    if (duration < 10) {
      return;
    } // Don't save very short durations

    const sessionTag = {
      session_id: `session-${Date.now()}`, // This should be the actual session ID
      tag_id: tagId,
      duration,
      created_at: new Date().toISOString(),
    };

    try {
      if (window.__TAURI__ && typeof window.__TAURI__.invoke === "function") {
        await window.__TAURI__.invoke("add_session_tag", sessionTag);
      }
    } catch (error) {
      console.error("Failed to save session tag:", error);
    }
  }

  // Methods to be called by PomodoroTimer
  onTimerStart() {
    // Start tracking all current tags
    this.currentTags.forEach((tag) => {
      this.startTagTracking(tag.id);
    });
  }

  onTimerPause() {
    // Stop tracking but keep the active session tags for resume
    this.activeSessionTags.forEach((startTime, tagId) => {
      const duration = Math.floor((Date.now() - startTime) / 1000);
      this.saveSessionTag(tagId, duration);
    });
    this.activeSessionTags.clear();
  }

  onTimerResume() {
    // Resume tracking all current tags
    this.currentTags.forEach((tag) => {
      this.startTagTracking(tag.id);
    });
  }

  onTimerStop() {
    // Stop tracking all tags and save durations
    this.activeSessionTags.forEach((startTime, tagId) => {
      const duration = Math.floor((Date.now() - startTime) / 1000);
      this.saveSessionTag(tagId, duration);
    });
    this.activeSessionTags.clear();
  }

  onTimerComplete() {
    // Same as stop but for completed sessions
    this.onTimerStop();
  }

  // Get current tags for external use
  getCurrentTags() {
    return [...this.currentTags];
  }

  // Set current tags programmatically
  setCurrentTags(tags) {
    this.currentTags = [...tags];
    this.updateStatusDisplay();
    this.renderTagList();
  }
}

// Initialize tag manager when DOM is loaded and Tauri is ready
function initializeTagManager() {
  // Wait a bit for Tauri to be available in development mode
  setTimeout(() => {
    window.tagManager = new TagManager();
  }, 100);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeTagManager);
} else {
  initializeTagManager();
}
