import { logger } from "../utils/logger.js";

class TagManager {
  constructor() {
    /** @type {any[]} */
    this.tags = [];
    /** @type {any[]} */
    this.currentTags = [];
    this.activeSessionTags = new Map();
    this.isDropdownOpen = false;
    this.selectedIcon = "ri-brain-line";

    this.initializeElements();
    this.bindEvents();
    this.loadTags();

    this.resetIconSelection();
  }

  initializeElements() {
    this.timerStatus = document.getElementById("timer-status");
    this.statusText = document.getElementById("status-text");
    this.statusIcon = document.getElementById("status-icon");
    this.dropdownArrow = document.getElementById("tag-dropdown-arrow");
    this.dropdownMenu = document.getElementById("tag-dropdown-menu");
    this.tagList = document.getElementById("tag-list");
    this.newTagName = /** @type {HTMLInputElement | null} */ (
      document.getElementById("new-tag-name")
    );
    this.iconSelector = document.getElementById("icon-selector-dropdown");
    this.selectedIconBtn = document.getElementById("selected-icon-btn");
    this.selectedIconDisplay = document.getElementById("selected-icon-display");
    this.createTagBtn = /** @type {HTMLButtonElement | null} */ (
      document.getElementById("create-tag-btn")
    );

    logger.debug("TagManager elements:", {
      timerStatus: !!this.timerStatus,
      dropdownMenu: !!this.dropdownMenu,
      tagList: !!this.tagList,
      newTagName: !!this.newTagName,
      iconSelector: !!this.iconSelector,
      createTagBtn: !!this.createTagBtn,
    });

    if (!this.dropdownMenu) {
      logger.error("Dropdown menu not found!");
    }
    if (!this.newTagName) {
      logger.error("New tag input not found!");
    }
  }

  bindEvents() {
    if (this.timerStatus && this.dropdownMenu) {
      const timerStatus = this.timerStatus;
      const dropdownMenu = this.dropdownMenu;

      timerStatus.addEventListener("click", () => {
        this.toggleDropdown();
      });

      document.addEventListener("click", (e) => {
        if (
          !timerStatus.contains(/** @type {Node | null} */ (e.target)) &&
          !dropdownMenu.contains(/** @type {Node | null} */ (e.target))
        ) {
          this.closeDropdown();
        }
      });
    }

    if (this.selectedIconBtn && this.iconSelector) {
      const selectedIconBtn = this.selectedIconBtn;
      const iconSelector = this.iconSelector;

      selectedIconBtn.addEventListener("click", () => {
        this.toggleIconSelector();
      });

      iconSelector.addEventListener("click", (e) => {
        const iconOption = /** @type {Element} */ (e.target).closest(".icon-option, .emoji-option");
        if (iconOption) {
          this.selectIcon(iconOption);
        }
      });

      document.addEventListener("click", (e) => {
        if (
          !selectedIconBtn.contains(/** @type {Node | null} */ (e.target)) &&
          !iconSelector.contains(/** @type {Node | null} */ (e.target))
        ) {
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

  /** @private */
  _loadTagsFromLocalStorage() {
    const savedTags = localStorage.getItem("presto-tags");
    if (savedTags) {
      try {
        const parsed = JSON.parse(savedTags);
        if (!Array.isArray(parsed)) {
          throw new TypeError("presto-tags must be an array");
        }
        const valid = parsed.filter(
          (t) =>
            t !== null &&
            typeof t === "object" &&
            typeof t.id === "string" &&
            t.id.length > 0 &&
            typeof t.name === "string" &&
            t.name.length > 0
        );
        if (valid.length !== parsed.length) {
          logger.error("TagManager: invalid tag entries detected, discarding corrupted data");
          localStorage.removeItem("presto-tags");
          this.tags = [];
        } else {
          this.tags = valid;
        }
      } catch (_parseError) {
        logger.error("TagManager: corrupted tags in localStorage, resetting");
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
    this.currentTags = this.currentTags.filter((ct) => this.tags.some((t) => t.id === ct.id));
    if (this.currentTags.length === 0) {
      this.currentTags = [this.tags[0]];
    }
    this.updateStatusDisplay();
    this.renderTagList();
  }

  async loadTags() {
    try {
      if (typeof window.__TAURI__?.core?.invoke !== "function") {
        logger.warn("Tauri is not available, using localStorage fallback");
        this._loadTagsFromLocalStorage();
        return;
      }

      this.tags = /** @type {any[]} */ (await window.__TAURI__.core.invoke("load_tags"));

      this.currentTags = this.currentTags.filter((ct) => this.tags.some((t) => t.id === ct.id));
      if (this.currentTags.length === 0 && this.tags.length > 0) {
        this.currentTags = [this.tags[0]];
      }
      this.updateStatusDisplay();
      this.renderTagList();
    } catch (error) {
      logger.error("Failed to load tags:", error);
      this._loadTagsFromLocalStorage();
    }
  }

  renderTagList() {
    const tagList = this.tagList;
    if (!tagList) {
      return;
    }
    tagList.innerHTML = "";

    this.tags.forEach((/** @type {any} */ tag) => {
      const tagItem = document.createElement("div");
      tagItem.className = "tag-item";
      tagItem.dataset.tagId = tag.id;

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

      tagItem.addEventListener("click", (e) => {
        if (!(/** @type {Element} */ (e.target).classList.contains("tag-item-delete"))) {
          e.stopPropagation();
          this.toggleTag(tag);
        }
      });

      const deleteBtn = tagItem.querySelector(".tag-item-delete");
      if (deleteBtn) {
        deleteBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          this.deleteTag(tag.id);
        });
      }

      tagList.appendChild(tagItem);
    });
  }

  /** @param {any} tag */
  toggleTag(tag) {
    const existingIndex = this.currentTags.findIndex((t) => t.id === tag.id);

    if (existingIndex !== -1) {
      this.currentTags.splice(existingIndex, 1);
      this.stopTagTracking(tag.id);
    } else {
      this.currentTags.push(tag);
      if (window.pomodoroTimer && window.pomodoroTimer.isRunning) {
        this.startTagTracking(tag.id);
      }
    }

    this.updateStatusDisplay();
    this.renderTagList();
  }

  async createNewTag() {
    if (!this.newTagName) {
      return;
    }
    const name = this.newTagName.value.trim();
    if (!name) {
      return;
    }

    const newTag = {
      id: `tag-${crypto.randomUUID()}`,
      name,
      icon: this.selectedIcon,
      color: "#4CAF50",
      created_at: new Date().toISOString(),
    };

    try {
      this.tags.push(newTag);

      if (typeof window.__TAURI__?.core?.invoke === "function") {
        await window.__TAURI__.core.invoke("save_tag", { tag: newTag });
      } else {
        this.saveTagsToLocalStorage();
      }

      this.renderTagList();

      this.newTagName.value = "";
      this.resetIconSelection();
      this.updateCreateButtonState();
    } catch (error) {
      this.tags = this.tags.filter((t) => t !== newTag);
      logger.error("Failed to create tag:", error);
    }
  }

  /** @param {any} tagId */
  async deleteTag(tagId) {
    if (this.tags.length <= 1) {
      alert("You cannot delete the last tag.");
      return;
    }

    try {
      if (typeof window.__TAURI__?.core?.invoke === "function") {
        await window.__TAURI__.core.invoke("delete_tag", { tag_id: tagId });
      }

      this.tags = this.tags.filter((t) => t.id !== tagId);
      this.currentTags = this.currentTags.filter((t) => t.id !== tagId);

      if (typeof window.__TAURI__?.core?.invoke !== "function") {
        this.saveTagsToLocalStorage();
      }

      this.stopTagTracking(tagId);

      if (this.currentTags.length === 0 && this.tags.length > 0) {
        this.currentTags = [this.tags[0]];
        if (window.pomodoroTimer && window.pomodoroTimer.isRunning) {
          this.startTagTracking(this.currentTags[0].id);
        }
      }

      this.updateStatusDisplay();
      this.renderTagList();
    } catch (error) {
      logger.error("Failed to delete tag:", error);
    }
  }

  toggleIconSelector() {
    if (!this.iconSelector) {
      return;
    }
    const isOpen = this.iconSelector.classList.contains("active");
    if (isOpen) {
      this.closeIconSelector();
    } else {
      this.openIconSelector();
    }
  }

  openIconSelector() {
    if (!this.iconSelector || !this.selectedIconBtn) {
      return;
    }
    this.iconSelector.classList.add("active");
    this.selectedIconBtn.classList.add("active");
  }

  closeIconSelector() {
    if (!this.iconSelector || !this.selectedIconBtn) {
      return;
    }
    this.iconSelector.classList.remove("active");
    this.selectedIconBtn.classList.remove("active");
  }

  /** @param {Element} iconOption */
  selectIcon(iconOption) {
    if (!this.iconSelector) {
      return;
    }
    this.iconSelector.querySelectorAll(".selected").forEach((el) => {
      el.classList.remove("selected");
    });

    iconOption.classList.add("selected");
    this.selectedIcon = /** @type {HTMLElement} */ (iconOption).dataset.icon ?? "";

    this.updateSelectedIconDisplay();
    this.updateCreateButtonState();
  }

  updateSelectedIconDisplay() {
    const iconDisplay = this.selectedIconDisplay;
    if (!iconDisplay) {
      return;
    }
    if (this.selectedIcon.startsWith("ri-")) {
      iconDisplay.className = this.selectedIcon;
      iconDisplay.textContent = "";
      iconDisplay.style.fontFamily = "remixicon";
    } else {
      iconDisplay.className = "";
      iconDisplay.textContent = this.selectedIcon;
      iconDisplay.style.fontFamily = "inherit";
    }
  }

  resetIconSelection() {
    if (!this.iconSelector) {
      return;
    }
    this.iconSelector.querySelectorAll(".selected").forEach((el) => {
      el.classList.remove("selected");
    });

    const defaultIcon = this.iconSelector.querySelector('[data-icon="ri-brain-line"]');
    if (defaultIcon) {
      defaultIcon.classList.add("selected");
      this.selectedIcon = "ri-brain-line";
      this.updateSelectedIconDisplay();
    }
  }

  updateCreateButtonState() {
    if (!this.newTagName || !this.createTagBtn) {
      return;
    }
    const hasName = this.newTagName.value.trim().length > 0;
    this.createTagBtn.disabled = !hasName;
  }

  saveTagsToLocalStorage() {
    try {
      localStorage.setItem("presto-tags", JSON.stringify(this.tags));
    } catch (error) {
      logger.error("Failed to save tags to localStorage:", error);
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
    logger.debug("Opening dropdown...");
    this.isDropdownOpen = true;
    this.timerStatus?.classList.add("active");
    this.dropdownMenu?.classList.add("active");
    logger.debug(
      "Dropdown classes added, menu visible:",
      this.dropdownMenu?.classList.contains("active")
    );
    this.loadTags();
  }

  closeDropdown() {
    this.isDropdownOpen = false;
    this.timerStatus?.classList.remove("active");
    this.dropdownMenu?.classList.remove("active");
  }

  updateStatusDisplay() {
    if (!this.statusText || !this.statusIcon) {
      return;
    }
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

      if (typeof tag.icon === "string" && tag.icon.startsWith("ri-")) {
        this.statusIcon.className = tag.icon;
        this.statusIcon.style.fontFamily = "remixicon";
        this.statusIcon.textContent = "";
      } else {
        this.statusIcon.style.fontFamily = "inherit";
        this.statusIcon.textContent = String(tag.icon ?? "");
        this.statusIcon.className = "";
      }
    } else {
      this.statusText.textContent = `${this.currentTags.length} Tags`;
      this.statusIcon.className = "ri-price-tag-3-line";
      this.statusIcon.style.fontFamily = "remixicon";
      this.statusIcon.textContent = "";
    }
  }

  /** @param {any} tagId */
  startTagTracking(tagId) {
    if (!this.activeSessionTags.has(tagId)) {
      this.activeSessionTags.set(tagId, Date.now());
    }
  }

  /** @param {any} tagId */
  stopTagTracking(tagId) {
    if (this.activeSessionTags.has(tagId)) {
      const startTime = this.activeSessionTags.get(tagId);
      const duration = Math.floor((Date.now() - startTime) / 1000);

      this.saveSessionTag(tagId, duration);
      this.activeSessionTags.delete(tagId);
    }
  }

  /** @param {any} tagId @param {number} duration */
  async saveSessionTag(tagId, duration) {
    if (duration < 10) {
      return;
    }

    const sessionTag = {
      session_id: `session-${Date.now()}`,
      tag_id: tagId,
      duration,
      created_at: new Date().toISOString(),
    };

    try {
      if (typeof window.__TAURI__?.core?.invoke === "function") {
        await window.__TAURI__.core.invoke("add_session_tag", { session_tag: sessionTag });
      }
    } catch (error) {
      logger.error("Failed to save session tag:", error);
    }
  }

  onTimerStart() {
    this.currentTags.forEach((tag) => {
      this.startTagTracking(tag.id);
    });
  }

  onTimerPause() {
    this.activeSessionTags.forEach((startTime, tagId) => {
      const duration = Math.floor((Date.now() - startTime) / 1000);
      this.saveSessionTag(tagId, duration);
    });
    this.activeSessionTags.clear();
  }

  onTimerResume() {
    this.currentTags.forEach((tag) => {
      this.startTagTracking(tag.id);
    });
  }

  onTimerStop() {
    this.activeSessionTags.forEach((startTime, tagId) => {
      const duration = Math.floor((Date.now() - startTime) / 1000);
      this.saveSessionTag(tagId, duration);
    });
    this.activeSessionTags.clear();
  }

  onTimerComplete() {
    this.onTimerStop();
  }

  /** @returns {any[]} */
  getCurrentTags() {
    return [...this.currentTags];
  }

  /** @param {any[]} tags */
  setCurrentTags(tags) {
    this.currentTags = [...tags];
    this.updateStatusDisplay();
    this.renderTagList();
  }
}

function initializeTagManager() {
  setTimeout(() => {
    window.tagManager = new TagManager();
  }, 100);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeTagManager);
} else {
  initializeTagManager();
}
