import { installNewtabReuse } from "./newtab_reuse.mjs";

installNewtabReuse(chrome);

import { installNewtabPosition } from "./newtab_position.mjs";

installNewtabPosition(chrome);

import {
    TAB_GROUP_BINDINGS_KEY,
    TAB_GROUP_CACHE_KEY,
    TAB_GROUP_CONFIG_KEY,
    TAB_GROUP_PAGE_MATCH_LEVEL,
    addCustomRule,
    deleteCustomRule,
    deleteGroup,
    deletePageMapping,
    findGroup,
    findGroupForUrl,
    findLiveGroupByName,
    isPinnableTab,
    normalizeCache,
    normalizeConfig,
    normalizeUrl,
    pinDomainRule,
    pinPageMapping,
    renameGroup as renameCachedGroup,
    reindexCache,
    removeUrlMapping,
    updateCustomRule,
    upsertGroup,
} from "./tab_group_rules.mjs";

const TAB_GROUP_ID_NONE = chrome.tabGroups.TAB_GROUP_ID_NONE;
const PIN_TO_GROUP_MENU_ID = "tab-groups:pin-to-current-group";
const PIN_DOMAIN_MENU_ID = "tab-groups:pin-domain";
const PIN_FULL_PATH_MENU_ID = "tab-groups:pin-full-path";
let operationQueue = Promise.resolve();
const pendingUngroups = new Map();

const enqueue = (operation) => {
    const result = operationQueue.then(operation);
    operationQueue = result.catch((error) => console.error("Tab group enhancement failed", error));
    return result;
};

const loadConfig = async () => {
    const stored = await chrome.storage.local.get(TAB_GROUP_CONFIG_KEY);
    return normalizeConfig(stored[TAB_GROUP_CONFIG_KEY]);
};

const loadCache = async () => {
    const stored = await chrome.storage.local.get(TAB_GROUP_CACHE_KEY);
    return normalizeCache(stored[TAB_GROUP_CACHE_KEY]);
};

const saveCache = (cache) => chrome.storage.local.set({ [TAB_GROUP_CACHE_KEY]: cache });

const loadBindings = async () => {
    const stored = await chrome.storage.session.get(TAB_GROUP_BINDINGS_KEY);
    const bindings = stored[TAB_GROUP_BINDINGS_KEY];
    return bindings && typeof bindings === "object" ? { ...bindings } : {};
};

const saveBindings = (bindings) =>
    chrome.storage.session.set({ [TAB_GROUP_BINDINGS_KEY]: bindings });

const supportedTab = (tab) =>
    Number.isInteger(tab?.id)
    && !tab.incognito
    && !tab.pinned
    && normalizeUrl(tab.url, "fragment") !== null;

const liveGroup = async (groupId) => {
    try {
        const group = await chrome.tabGroups.get(Number(groupId));
        return group?.shared ? null : group;
    } catch {
        return null;
    }
};

const updateSavedGroup = (saved, title, color, now = Date.now()) => {
    saved.title = typeof title === "string" ? title : "";
    saved.color = typeof color === "string" ? color : "grey";
    saved.updatedAt = now;
};

const groupName = (group) => typeof group?.title === "string" ? group.title : "";

const resolveGroup = (group, cache, bindings, liveGroups) => {
    const bindingKey = String(group.id);
    const name = groupName(group);
    if (!name) {
        delete bindings[bindingKey];
        return null;
    }

    const hasExistingBinding = Object.hasOwn(bindings, bindingKey);
    const existingId = bindings[bindingKey];
    if (
        hasExistingBinding
        && existingId
        && existingId !== name
        && findGroup(cache, existingId)
    ) {
        const otherExistingGroup = findLiveGroupByName(
            liveGroups.filter((candidate) => candidate.id !== group.id),
            existingId,
        );
        if (!otherExistingGroup) renameCachedGroup(cache, existingId, name);
    }
    bindings[bindingKey] = name;
    upsertGroup(cache, group);
    return name;
};

const findLiveBinding = async (logicalId, cache, bindings) => {
    for (const [groupId, boundId] of Object.entries(bindings)) {
        if (boundId !== logicalId) continue;
        const group = await liveGroup(groupId);
        if (group) {
            if (groupName(group) === logicalId) return group;
            const groups = await chrome.tabGroups.query({});
            const resolvedId = resolveGroup(group, cache, bindings, groups);
            if (resolvedId) {
                if (resolvedId === logicalId || !findGroup(cache, logicalId)) return group;
                continue;
            }
        }
        delete bindings[groupId];
    }

    // Session bindings can briefly be absent while the service worker or Chrome is
    // restoring its event state. The Chrome group name is the persistent identity,
    // so recover the live binding before deciding that a new group is required.
    const group = findLiveGroupByName(await chrome.tabGroups.query({}), logicalId);
    if (group) bindings[String(group.id)] = logicalId;
    return group;
};

const restoreTabToGroup = async (tab, saved, cache, bindings) => {
    const group = await findLiveBinding(saved.id, cache, bindings);
    if (group) {
        if (tab.windowId !== group.windowId) {
            await chrome.tabs.move(tab.id, { windowId: group.windowId, index: -1 });
        }
        await chrome.tabs.group({ tabIds: tab.id, groupId: group.id });
        if (tab.active) {
            await chrome.tabs.update(tab.id, { active: true });
            await chrome.windows.update(group.windowId, { focused: true });
        }
        return;
    }

    const groupId = await chrome.tabs.group({
        tabIds: tab.id,
        createProperties: { windowId: tab.windowId },
    });
    await chrome.tabGroups.update(groupId, {
        title: saved.title,
        color: saved.color,
    });
    bindings[String(groupId)] = saved.id;
};

const processTab = async (tabId, suppliedTab = null) => {
    const config = await loadConfig();
    if (!config.enabled) return;
    const tab = suppliedTab ?? await chrome.tabs.get(tabId);
    if (!supportedTab(tab)) return;
    const grouped = Number.isInteger(tab.groupId) && tab.groupId !== TAB_GROUP_ID_NONE;
    if (grouped) return;
    const cache = await loadCache();
    const bindings = await loadBindings();

    const saved = findGroupForUrl(cache, tab.url, TAB_GROUP_PAGE_MATCH_LEVEL);
    if (saved) {
        await restoreTabToGroup(tab, saved, cache, bindings);
        await Promise.all([saveCache(cache), saveBindings(bindings)]);
    }
};

const confirmTabUngrouped = async (tabId, url) => {
    pendingUngroups.delete(tabId);
    let tab;
    try {
        tab = await chrome.tabs.get(tabId);
    } catch {
        return;
    }
    if (tab.groupId !== TAB_GROUP_ID_NONE) return;
    const config = await loadConfig();
    if (!config.enabled) return;
    const cache = await loadCache();
    if (removeUrlMapping(cache, url || tab.url, TAB_GROUP_PAGE_MATCH_LEVEL)) {
        await saveCache(cache);
    }
};

const scheduleUngroupCheck = (tabId, url) => {
    clearTimeout(pendingUngroups.get(tabId));
    const timer = setTimeout(
        () => void enqueue(() => confirmTabUngrouped(tabId, url)),
        250,
    );
    pendingUngroups.set(tabId, timer);
};

const reconcileAll = async () => {
    const config = await loadConfig();
    const cache = reindexCache(await loadCache(), TAB_GROUP_PAGE_MATCH_LEVEL);
    if (!config.enabled) {
        await saveCache(cache);
        return;
    }
    const bindings = await loadBindings();
    const groups = await chrome.tabGroups.query({});

    for (const group of groups.sort((left, right) => left.id - right.id)) {
        if (group.shared) continue;
        let window;
        try {
            window = await chrome.windows.get(group.windowId);
        } catch {
            continue;
        }
        if (window.type !== "normal" || window.incognito) continue;
        resolveGroup(group, cache, bindings, groups);
    }

    await Promise.all([saveCache(cache), saveBindings(bindings)]);
    const ungrouped = await chrome.tabs.query({ groupId: TAB_GROUP_ID_NONE });
    for (const tab of ungrouped) {
        if (supportedTab(tab)) await processTab(tab.id, tab);
    }
};

const updateGroupMetadata = async (group) => {
    const config = await loadConfig();
    if (!config.enabled) return;
    const current = await liveGroup(group.id);
    if (!current) {
        await removeGroupBinding(group.id);
        return;
    }
    const cache = await loadCache();
    const bindings = await loadBindings();
    const groups = await chrome.tabGroups.query({});
    resolveGroup(current, cache, bindings, groups);
    await Promise.all([saveCache(cache), saveBindings(bindings)]);
};

const removeGroupBinding = async (groupId) => {
    const bindings = await loadBindings();
    delete bindings[String(groupId)];
    await saveBindings(bindings);
};

const renameGroup = async (id, title) => {
    const normalizedTitle = title.trim();
    if (!normalizedTitle) throw new Error("Tab group title cannot be empty");
    const cache = await loadCache();
    const saved = renameCachedGroup(cache, id, normalizedTitle);
    if (!saved) throw new Error("Saved tab group was not found");
    const bindings = await loadBindings();
    const group = await findLiveBinding(id, cache, bindings);
    for (const groupId of Object.keys(bindings)) {
        if (bindings[groupId] === id) bindings[groupId] = saved.id;
    }
    await Promise.all([saveCache(cache), saveBindings(bindings)]);
    if (group) await chrome.tabGroups.update(group.id, { title: saved.title });
};

const setGroupColor = async (id, color) => {
    const cache = await loadCache();
    const saved = findGroup(cache, id);
    if (!saved) throw new Error("Saved tab group was not found");
    updateSavedGroup(saved, saved.title, color);
    const bindings = await loadBindings();
    const group = await findLiveBinding(id, cache, bindings);
    await Promise.all([saveCache(cache), saveBindings(bindings)]);
    if (group) await chrome.tabGroups.update(group.id, { color });
};

const deleteSavedGroup = async (id) => {
    const cache = await loadCache();
    if (!deleteGroup(cache, id)) return;
    await saveCache(cache);
    const bindings = await loadBindings();
    for (const [groupId, logicalId] of Object.entries(bindings)) {
        if (logicalId !== id) continue;
        const tabs = await chrome.tabs.query({ groupId: Number(groupId) });
        const tabIds = tabs.map((tab) => tab.id).filter(Number.isInteger);
        delete bindings[groupId];
        if (tabIds.length > 0) await chrome.tabs.ungroup(tabIds);
    }
    await saveBindings(bindings);
};

const addRule = async (groupId, pattern) => {
    const cache = await loadCache();
    const ruleId = globalThis.crypto?.randomUUID?.()
        ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const rule = addCustomRule(cache, groupId, ruleId, pattern);
    if (!rule) throw new Error("Saved tab group or custom rule was invalid");
    await saveCache(cache);
    return rule;
};

const updateRule = async (groupId, ruleId, pattern) => {
    const cache = await loadCache();
    const rule = updateCustomRule(cache, groupId, ruleId, pattern);
    if (!rule) throw new Error("Saved custom rule was not found or was invalid");
    await saveCache(cache);
    return rule;
};

const deleteRule = async (groupId, ruleId) => {
    const cache = await loadCache();
    if (!deleteCustomRule(cache, groupId, ruleId)) return;
    await saveCache(cache);
};

const deleteSavedPage = async (groupId, url) => {
    const cache = await loadCache();
    if (!deletePageMapping(cache, groupId, url, TAB_GROUP_PAGE_MATCH_LEVEL)) return;
    await saveCache(cache);

    const key = normalizeUrl(url, TAB_GROUP_PAGE_MATCH_LEVEL);
    const bindings = await loadBindings();
    for (const [liveGroupId, logicalId] of Object.entries(bindings)) {
        if (logicalId !== groupId) continue;
        const group = await liveGroup(liveGroupId);
        if (!group) continue;
        const tabs = await chrome.tabs.query({ groupId: group.id });
        const tabIds = tabs
            .filter((tab) => supportedTab(tab)
                && normalizeUrl(tab.url, TAB_GROUP_PAGE_MATCH_LEVEL) === key)
            .map((tab) => tab.id);
        if (tabIds.length > 0) await chrome.tabs.ungroup(tabIds);
    }
};

const pinTabToCurrentGroup = async (tabId, mode) => {
    const config = await loadConfig();
    if (!config.enabled) return;

    let tab;
    try {
        tab = await chrome.tabs.get(tabId);
    } catch {
        return;
    }
    if (!isPinnableTab(tab, TAB_GROUP_ID_NONE)) return;

    let window;
    try {
        window = await chrome.windows.get(tab.windowId);
    } catch {
        return;
    }
    if (window.type !== "normal" || window.incognito) return;

    const group = await liveGroup(tab.groupId);
    if (!group) return;
    const [cache, bindings] = await Promise.all([loadCache(), loadBindings()]);
    const liveGroups = await chrome.tabGroups.query({});
    const logicalId = resolveGroup(group, cache, bindings, liveGroups);
    if (!logicalId) {
        await saveBindings(bindings);
        return;
    }
    let result = null;
    if (mode === "domain") {
        const ruleId = globalThis.crypto?.randomUUID?.()
            ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
        result = pinDomainRule(cache, logicalId, tab.url, ruleId);
    } else if (mode === "full_path") {
        result = pinPageMapping(
            cache,
            logicalId,
            tab,
            TAB_GROUP_PAGE_MATCH_LEVEL,
        );
    }
    if (!result) {
        await Promise.all([saveCache(cache), saveBindings(bindings)]);
        return;
    }
    await Promise.all([saveCache(cache), saveBindings(bindings)]);
};

const createContextMenuItem = (properties, onCreated = () => {}) => {
    try {
        chrome.contextMenus.create(properties, () => {
            const failed = Boolean(chrome.runtime.lastError);
            if (!failed) onCreated();
        });
    } catch {
        // Chrome versions before tab context-menu support reject the "tab" context.
    }
};

const ensureContextMenuItem = (id, properties, onReady = () => {}) => {
    try {
        chrome.contextMenus.update(id, properties, () => {
            const missing = Boolean(chrome.runtime.lastError);
            if (missing) {
                createContextMenuItem({ id, ...properties }, onReady);
            } else {
                onReady();
            }
        });
    } catch {
        createContextMenuItem({ id, ...properties }, onReady);
    }
};

const ensurePinContextMenuChildren = () => {
    for (const [id, title] of [
        [PIN_DOMAIN_MENU_ID, "Pin domain"],
        [PIN_FULL_PATH_MENU_ID, "Pin full path"],
    ]) {
        ensureContextMenuItem(id, {
            title,
            parentId: PIN_TO_GROUP_MENU_ID,
            contexts: ["tab"],
            visible: true,
        });
    }
};

const syncPinContextMenu = (enabled) => {
    if (!chrome.contextMenus) return;
    if (!enabled) {
        for (const id of [PIN_DOMAIN_MENU_ID, PIN_FULL_PATH_MENU_ID, PIN_TO_GROUP_MENU_ID]) {
            try {
                chrome.contextMenus.remove(id, () => void chrome.runtime.lastError);
            } catch {
                // The menu is unavailable or was never registered.
            }
        }
        return;
    }

    ensureContextMenuItem(PIN_TO_GROUP_MENU_ID, {
        title: "Tab group enhancement",
        contexts: ["tab"],
        visible: true,
    }, ensurePinContextMenuChildren);
};

void loadConfig()
    .then((config) => syncPinContextMenu(config.enabled))
    .catch((error) => console.warn("Unable to synchronize Pin to group menu", error));

chrome.tabs.onCreated.addListener((tab) => {
    if (Number.isInteger(tab.id)) void enqueue(() => processTab(tab.id, tab));
});
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
    if (changeInfo.groupId === TAB_GROUP_ID_NONE) {
        scheduleUngroupCheck(tabId, tab.url);
        return;
    }
    if (Number.isInteger(tab.groupId) && tab.groupId !== TAB_GROUP_ID_NONE) return;
    void enqueue(() => processTab(tabId, tab));
});
chrome.tabs.onRemoved.addListener((tabId) => {
    clearTimeout(pendingUngroups.get(tabId));
    pendingUngroups.delete(tabId);
});
chrome.tabGroups.onCreated.addListener((group) => {
    void enqueue(() => updateGroupMetadata(group));
});
chrome.tabGroups.onUpdated.addListener((group) => {
    void enqueue(() => updateGroupMetadata(group));
});
chrome.tabGroups.onRemoved.addListener((group) => {
    void enqueue(() => removeGroupBinding(group.id));
});
chrome.runtime.onStartup.addListener(() => void enqueue(reconcileAll));
chrome.runtime.onInstalled.addListener(() => void enqueue(reconcileAll));

chrome.storage.onChanged.addListener((changes, areaName) => {
    if (areaName !== "local" || !changes[TAB_GROUP_CONFIG_KEY]) return;
    const oldConfig = normalizeConfig(changes[TAB_GROUP_CONFIG_KEY].oldValue);
    const newConfig = normalizeConfig(changes[TAB_GROUP_CONFIG_KEY].newValue);
    syncPinContextMenu(newConfig.enabled);
    if (newConfig.enabled && !oldConfig.enabled) {
        void enqueue(reconcileAll);
    }
});

chrome.contextMenus?.onClicked.addListener((info, tab) => {
    if (!Number.isInteger(tab?.id)) return;
    if (info.menuItemId === PIN_DOMAIN_MENU_ID) {
        void enqueue(() => pinTabToCurrentGroup(tab.id, "domain"));
    } else if (info.menuItemId === PIN_FULL_PATH_MENU_ID) {
        void enqueue(() => pinTabToCurrentGroup(tab.id, "full_path"));
    }
});

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    let task;
    switch (message?.type) {
        case "tab-groups:rename":
            task = enqueue(() => renameGroup(message.id, message.title));
            break;
        case "tab-groups:set-color":
            task = enqueue(() => setGroupColor(message.id, message.color));
            break;
        case "tab-groups:delete":
            task = enqueue(() => deleteSavedGroup(message.id));
            break;
        case "tab-groups:add-rule":
            task = enqueue(() => addRule(message.groupId, message.pattern));
            break;
        case "tab-groups:update-rule":
            task = enqueue(() => updateRule(message.groupId, message.ruleId, message.pattern));
            break;
        case "tab-groups:delete-rule":
            task = enqueue(() => deleteRule(message.groupId, message.ruleId));
            break;
        case "tab-groups:delete-page":
            task = enqueue(() => deleteSavedPage(message.groupId, message.url));
            break;
        case "tab-groups:refresh":
            task = enqueue(reconcileAll);
            break;
        default:
            return false;
    }
    task.then(
        (result) => sendResponse({ ok: true, result }),
        (error) => sendResponse({ ok: false, error: String(error?.message ?? error) }),
    );
    return true;
});
