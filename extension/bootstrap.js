// Allow standard browser and system shortcuts (such as Cmd/Ctrl + L for address bar,
// Cmd/Ctrl + P for print / macOS App Shortcuts, Cmd/Ctrl + R for reload, devtools, etc.)
// to pass through to the browser instead of being intercepted and preventDefaulted by eframe/canvas.
const shouldPassthroughShortcut = (event) => {
    const isCmdOrCtrl = event.metaKey || event.ctrlKey;
    const key = event.key ? event.key.toLowerCase() : "";

    // Keep Cmd/Ctrl + S for egui state saving
    if (isCmdOrCtrl && key === "s") {
        return false;
    }

    return (
        // Address bar (Cmd/Ctrl + L), new tab (Cmd/Ctrl + T), Print / custom menu shortcuts (Cmd/Ctrl + P),
        // Reload (Cmd/Ctrl + R), History (Cmd/Ctrl + H), Downloads (Cmd/Ctrl + J)
        (isCmdOrCtrl && (key === "l" || key === "t" || key === "p" || key === "r" || key === "h" || key === "j")) ||
        // Bookmarks (Cmd/Ctrl + Shift + B/O), Devtools (Cmd/Ctrl + Shift/Option + I/J/C), Hard reload (Cmd/Ctrl + Shift + R)
        (isCmdOrCtrl && event.shiftKey && (key === "b" || key === "o" || key === "delete" || key === "i" || key === "j" || key === "c" || key === "r")) ||
        (isCmdOrCtrl && event.altKey && (key === "i" || key === "j" || key === "c")) ||
        key === "f12" ||
        key === "f5"
    );
};

window.addEventListener(
    "keydown",
    (event) => {
        if (shouldPassthroughShortcut(event)) {
            // Stop eframe/canvas from receiving this event and calling preventDefault()
            event.stopImmediatePropagation();
        }
    },
    true // Capture phase to intercept before reaching canvas listeners
);

window.addEventListener(
    "keyup",
    (event) => {
        if (shouldPassthroughShortcut(event)) {
            event.stopImmediatePropagation();
        }
    },
    true
);

import init from "./real-useful-newtabs.js";
import { NEWTAB_REUSE_KEY } from "./newtab_reuse.mjs";
import { NEWTAB_POSITION_KEY } from "./newtab_position.mjs";

const APP_STATE_KEY = "app_state";
const TAB_GROUP_CACHE_KEY = "tab_group_cache_v1";
const TAB_GROUP_CONFIG_KEY = "tab_group_config_v1";
const EGUI_MEMORY_KEY = "egui_memory_ron";
const LEGACY_STORAGE_KEYS = ["app"];
const LAYOUT_PREFIX = "layout:";
const writerId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
let initialStorage = {};
let appStateWriteQueue = Promise.resolve();
let layoutWriteQueue = Promise.resolve();
const appStateListeners = new Set();
const tabGroupListeners = new Set();
const pendingLayouts = new Map();
let latestRemoteAppState = null;

const readLocalStorage = (key) => {
    try {
        return globalThis.localStorage?.getItem(key) ?? null;
    } catch (error) {
        console.warn(`Unable to read legacy localStorage key "${key}"`, error);
        return null;
    }
};

const writeLocalStorage = (values) => {
    for (const [key, value] of Object.entries(values)) {
        globalThis.localStorage?.setItem(key, JSON.stringify(value));
    }
};

const decodeStoredValue = (key, value) => {
    if (key === APP_STATE_KEY && value && typeof value === "object") {
        return typeof value.stateRon === "string" ? value.stateRon : null;
    }
    return typeof value === "string" ? value : null;
};

const readAllLocalStorage = () => {
    const values = {};
    try {
        for (let index = 0; index < (globalThis.localStorage?.length ?? 0); index += 1) {
            const key = globalThis.localStorage.key(index);
            if (
                !key
                || (
                    key !== APP_STATE_KEY
                    && key !== TAB_GROUP_CACHE_KEY
                    && key !== TAB_GROUP_CONFIG_KEY
                    && !LEGACY_STORAGE_KEYS.includes(key)
                    && !key.startsWith(LAYOUT_PREFIX)
                )
            ) {
                continue;
            }
            const raw = globalThis.localStorage.getItem(key);
            try {
                values[key] = JSON.parse(raw);
            } catch {
                values[key] = raw;
            }
        }
    } catch (error) {
        console.warn("Unable to read localStorage", error);
    }
    return values;
};

const prepareStorage = async () => {
    const chromeStorage = globalThis.chrome?.storage?.local;
    if (!chromeStorage) {
        console.warn("chrome.storage.local is unavailable; falling back to localStorage");
        initialStorage = readAllLocalStorage();
        return;
    }

    let stored;
    try {
        stored = await chromeStorage.get(null);
    } catch (error) {
        console.error("Unable to load chrome.storage.local; falling back to localStorage", error);
        initialStorage = readAllLocalStorage();
        return;
    }

    const storedEguiMemory = typeof stored[EGUI_MEMORY_KEY] === "string"
        ? stored[EGUI_MEMORY_KEY]
        : null;
    if (storedEguiMemory !== null) {
        try {
            const localStorage = globalThis.localStorage;
            if (!localStorage) {
                throw new Error("localStorage is unavailable");
            }
            if (localStorage.getItem(EGUI_MEMORY_KEY) === null) {
                localStorage.setItem(EGUI_MEMORY_KEY, storedEguiMemory);
            }
            await chromeStorage.remove(EGUI_MEMORY_KEY);
            delete stored[EGUI_MEMORY_KEY];
        } catch (error) {
            console.warn("Unable to restore eframe UI memory to localStorage", error);
        }
    }

    const migrated = {};
    for (const key of [APP_STATE_KEY, ...LEGACY_STORAGE_KEYS]) {
        const hasStoredValue = key === APP_STATE_KEY
            ? typeof stored[key]?.stateRon === "string" || typeof stored[key] === "string"
            : typeof stored[key] === "string";
        if (!hasStoredValue) {
            const legacyValue = readLocalStorage(key);
            if (legacyValue !== null) {
                stored[key] = legacyValue;
                migrated[key] = legacyValue;
            }
        }
    }

    let migrationSucceeded = true;
    if (Object.keys(migrated).length > 0) {
        try {
            await chromeStorage.set(migrated);
        } catch (error) {
            migrationSucceeded = false;
            console.error("Unable to migrate localStorage data to chrome.storage.local", error);
        }
    }

    initialStorage = stored;

    for (const key of [APP_STATE_KEY, ...LEGACY_STORAGE_KEYS]) {
        if (!(key in stored) || (!migrationSucceeded && key in migrated)) {
            continue;
        }
        try {
            globalThis.localStorage?.removeItem(key);
        } catch (error) {
            console.warn(`Unable to remove migrated localStorage key "${key}"`, error);
        }
    }
};

globalThis.__newtabs_get_storage_value = (key) => decodeStoredValue(key, initialStorage[key]);

globalThis.__newtabs_save_app_state = (stateRon, tabGroupEnhancementEnabled, keepNewtabRightmost, reuseNewtab) => {
    const value = { writerId, stateRon };
    const tabGroupConfig = {
        enabled: tabGroupEnhancementEnabled === true,
    };
    const chromeStorage = globalThis.chrome?.storage?.local;
    const write = appStateWriteQueue.then(() => {
        initialStorage[NEWTAB_REUSE_KEY] = reuseNewtab === true;
        initialStorage[NEWTAB_POSITION_KEY] = keepNewtabRightmost === true;
        initialStorage[APP_STATE_KEY] = value;
        initialStorage[TAB_GROUP_CONFIG_KEY] = tabGroupConfig;
        if (chromeStorage) {
            return chromeStorage.set({
                [APP_STATE_KEY]: value,
                [NEWTAB_REUSE_KEY]: reuseNewtab === true,
                [NEWTAB_POSITION_KEY]: keepNewtabRightmost === true,
                [TAB_GROUP_CONFIG_KEY]: tabGroupConfig,
            });
        }
        writeLocalStorage({
            [APP_STATE_KEY]: value,
            [NEWTAB_POSITION_KEY]: keepNewtabRightmost === true,
            [NEWTAB_REUSE_KEY]: reuseNewtab === true,
            [TAB_GROUP_CONFIG_KEY]: tabGroupConfig,
        });
    });

    appStateWriteQueue = write.catch((error) => {
        console.error("Unable to save real-useful-newtabs AppState", error);
    });
    return write;
};

const persistLayout = (key) => {
    const pending = pendingLayouts.get(key);
    if (!pending) return Promise.resolve();
    clearTimeout(pending.timer);
    pendingLayouts.delete(key);
    initialStorage[key] = pending.value;
    const chromeStorage = globalThis.chrome?.storage?.local;
    const write = layoutWriteQueue.then(() => {
        if (chromeStorage) return chromeStorage.set({ [key]: pending.value });
        writeLocalStorage({ [key]: pending.value });
    });
    layoutWriteQueue = write.catch((error) => console.error(`Unable to save layout ${key}`, error));
    return write;
};

globalThis.__newtabs_queue_layout = (key, layoutRon) => {
    const previous = pendingLayouts.get(key);
    if (previous) clearTimeout(previous.timer);
    const timer = setTimeout(() => persistLayout(key), 1000);
    pendingLayouts.set(key, { value: layoutRon, timer });
};

globalThis.__newtabs_remove_layout = (key) => {
    const pending = pendingLayouts.get(key);
    if (pending) clearTimeout(pending.timer);
    pendingLayouts.delete(key);
    delete initialStorage[key];
    const chromeStorage = globalThis.chrome?.storage?.local;
    if (chromeStorage) {
        layoutWriteQueue = layoutWriteQueue.then(() => chromeStorage.remove(key));
    } else {
        globalThis.localStorage?.removeItem(key);
    }
};

globalThis.__newtabs_subscribe_app_state = (callback) => {
    appStateListeners.add(callback);
    if (latestRemoteAppState) callback(latestRemoteAppState);
};

const notifyTabGroups = (value) => {
    initialStorage[TAB_GROUP_CACHE_KEY] = value;
    const groups = Array.isArray(value?.groups) ? value.groups : [];
    for (const listener of tabGroupListeners) listener(groups);
};

globalThis.__newtabs_load_tab_groups = async () => {
    try {
        await globalThis.chrome?.runtime?.sendMessage?.({ type: "tab-groups:refresh" });
    } catch (error) {
        console.warn("Unable to refresh saved tab groups", error);
    }
    const chromeStorage = globalThis.chrome?.storage?.local;
    if (chromeStorage) {
        const stored = await chromeStorage.get(TAB_GROUP_CACHE_KEY);
        initialStorage[TAB_GROUP_CACHE_KEY] = stored[TAB_GROUP_CACHE_KEY];
    }
    return initialStorage[TAB_GROUP_CACHE_KEY]?.groups ?? [];
};

globalThis.__newtabs_subscribe_tab_groups = (callback) => {
    tabGroupListeners.add(callback);
    if (initialStorage[TAB_GROUP_CACHE_KEY]) {
        notifyTabGroups(initialStorage[TAB_GROUP_CACHE_KEY]);
    }
};

const sendTabGroupCommand = async (type, details) => {
    if (!globalThis.chrome?.runtime?.sendMessage) {
        throw new Error("Chrome extension runtime is unavailable");
    }
    const response = await globalThis.chrome.runtime.sendMessage({ type, ...details });
    if (response?.ok === false) throw new Error(response.error ?? "Tab group command failed");
    return response;
};

globalThis.__newtabs_rename_tab_group = (id, title) =>
    sendTabGroupCommand("tab-groups:rename", { id, title });
globalThis.__newtabs_set_tab_group_color = (id, color) =>
    sendTabGroupCommand("tab-groups:set-color", { id, color });
globalThis.__newtabs_delete_tab_group = (id) =>
    sendTabGroupCommand("tab-groups:delete", { id });
globalThis.__newtabs_add_tab_group_rule = (groupId, pattern) =>
    sendTabGroupCommand("tab-groups:add-rule", { groupId, pattern });
globalThis.__newtabs_update_tab_group_rule = (groupId, ruleId, pattern) =>
    sendTabGroupCommand("tab-groups:update-rule", { groupId, ruleId, pattern });
globalThis.__newtabs_delete_tab_group_rule = (groupId, ruleId) =>
    sendTabGroupCommand("tab-groups:delete-rule", { groupId, ruleId });
globalThis.__newtabs_delete_tab_group_page = (groupId, url) =>
    sendTabGroupCommand("tab-groups:delete-page", { groupId, url });

const notifyAppState = (value) => {
    if (!value || value.writerId === writerId || typeof value.stateRon !== "string") return;
    initialStorage[APP_STATE_KEY] = value;
    latestRemoteAppState = value.stateRon;
    for (const listener of appStateListeners) listener(value.stateRon);
};

globalThis.chrome?.storage?.onChanged?.addListener((changes, areaName) => {
    if (areaName !== "local") return;
    if (changes[APP_STATE_KEY]) notifyAppState(changes[APP_STATE_KEY].newValue);
    if (changes[TAB_GROUP_CACHE_KEY]) notifyTabGroups(changes[TAB_GROUP_CACHE_KEY].newValue);
});

window.addEventListener("storage", (event) => {
    if (event.key !== APP_STATE_KEY || !event.newValue) return;
    try {
        notifyAppState(JSON.parse(event.newValue));
    } catch (error) {
        console.warn("Unable to parse synchronized AppState", error);
    }
});

const flushLayouts = () => {
    for (const key of [...pendingLayouts.keys()]) persistLayout(key);
};
window.addEventListener("pagehide", flushLayouts);
document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") flushLayouts();
});

globalThis.__newtabs_load_bookmarks = async () => {
    if (!globalThis.chrome?.bookmarks?.getTree) {
        throw new Error("Chrome bookmarks API is unavailable");
    }

    return globalThis.chrome.bookmarks.getTree();
};

globalThis.__newtabs_load_favicon = async (pageUrl, size) => {
    if (!globalThis.chrome?.runtime?.getURL) {
        throw new Error("Chrome favicon API is unavailable");
    }

    const parsedPageUrl = new URL(pageUrl);
    if (parsedPageUrl.protocol !== "http:" && parsedPageUrl.protocol !== "https:") {
        throw new Error("Only HTTP and HTTPS pages have supported favicons");
    }

    const faviconUrl = new URL(globalThis.chrome.runtime.getURL("/_favicon/"));
    faviconUrl.searchParams.set("pageUrl", parsedPageUrl.toString());
    faviconUrl.searchParams.set("size", String(size));

    const image = new Image();
    image.decoding = "async";
    image.src = faviconUrl.toString();
    await image.decode();

    const canvas = document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) {
        throw new Error("Unable to create favicon canvas context");
    }

    context.clearRect(0, 0, size, size);
    context.drawImage(image, 0, 0, size, size);
    const imageData = context.getImageData(0, 0, size, size);
    return {
        width: imageData.width,
        height: imageData.height,
        pixels: imageData.data,
    };
};

globalThis.__newtabs_rename_bookmark = async (id, title) => {
    if (!globalThis.chrome?.bookmarks?.update) {
        throw new Error("Chrome bookmarks API is unavailable");
    }

    return globalThis.chrome.bookmarks.update(id, { title });
};

globalThis.__newtabs_delete_bookmark = async (id, isFolder) => {
    if (!globalThis.chrome?.bookmarks) {
        throw new Error("Chrome bookmarks API is unavailable");
    }

    if (isFolder && globalThis.chrome.bookmarks.removeTree) {
        return globalThis.chrome.bookmarks.removeTree(id);
    } else {
        return globalThis.chrome.bookmarks.remove(id);
    }
};

globalThis.__newtabs_set_theme = (dark) => {
    const scheme = dark ? "dark" : "light";
    document.documentElement.style.colorScheme = scheme;
    document.body?.style && (document.body.style.colorScheme = scheme);
};

globalThis.__newtabs_open_url = (url, newTab, active) => {
    if (!newTab) {
        globalThis.location.assign(url);
        return;
    }

    const createTab = globalThis.chrome?.tabs?.create;
    if (createTab) {
        try {
            const result = createTab.call(globalThis.chrome.tabs, { url, active });
            result?.catch?.((error) => {
                console.error("Unable to create Chrome tab", error);
                globalThis.open(url, "_blank");
            });
            return;
        } catch (error) {
            console.error("Unable to create Chrome tab", error);
        }
    }

    const opened = globalThis.open(url, "_blank");
    if (active) opened?.focus();
};

globalThis.__newtabs_move_bookmark = async (id, parentId, index) => {
    if (!globalThis.chrome?.bookmarks?.move) {
        throw new Error("Chrome bookmarks API is unavailable");
    }

    return globalThis.chrome.bookmarks.move(id, { parentId, index });
};

const shouldStartNewtabs = await (globalThis.__newtabs_tab_reuse_check ?? Promise.resolve(false));

if (!shouldStartNewtabs) {
    startNewtabs();
}

async function startNewtabs() {
    try {
        await prepareStorage();
        await init();
    } catch (error) {
        console.error("Failed to initialize real-useful-newtabs", error);
        const status = document.getElementById("loading_status");
        if (status) {
            status.textContent = "real-useful-newtabs failed to load. Check the console.";
        }
    }
}
