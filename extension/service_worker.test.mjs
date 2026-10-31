import test from "node:test";
import assert from "node:assert/strict";

import {
    TAB_GROUP_BINDINGS_KEY,
    TAB_GROUP_CACHE_KEY,
    TAB_GROUP_CONFIG_KEY,
} from "./tab_group_rules.mjs";

const event = () => {
    const listeners = [];
    return {
        addListener: (listener) => listeners.push(listener),
        dispatch: (...args) => listeners.forEach((listener) => listener(...args)),
        listeners,
    };
};

const copy = (value) => value === undefined ? undefined : structuredClone(value);

const local = {};
const session = {};
const groups = new Map();
const tabs = new Map();
const groupCalls = [];
const groupUpdates = [];
const tabMoves = [];
const windowUpdates = [];
let nextGroupId = 100;

const tabCreated = event();
const tabUpdated = event();
const tabRemoved = event();
const groupCreated = event();
const groupUpdated = event();
const groupRemoved = event();
const runtimeStartup = event();
const runtimeInstalled = event();
const runtimeMessage = event();
const storageChanged = event();

const storageArea = (values) => ({
    get: async (key) => ({ [key]: copy(values[key]) }),
    set: async (updates) => {
        for (const [key, value] of Object.entries(updates)) values[key] = copy(value);
    },
});

globalThis.chrome = {
    runtime: {
        lastError: null,
        getURL: (path) => `chrome-extension://test/${path}`,
        onStartup: runtimeStartup,
        onInstalled: runtimeInstalled,
        onMessage: runtimeMessage,
    },
    storage: {
        local: storageArea(local),
        session: storageArea(session),
        onChanged: storageChanged,
    },
    tabs: {
        onActivated: event(),
        onMoved: event(),
        onAttached: event(),
        onDetached: event(),
        onCreated: tabCreated,
        onUpdated: tabUpdated,
        onRemoved: tabRemoved,
        get: async (tabId) => {
            const tab = tabs.get(tabId);
            if (!tab) throw new Error("Tab not found");
            return copy(tab);
        },
        query: async ({ groupId } = {}) => [...tabs.values()]
            .filter((tab) => groupId === undefined || tab.groupId === groupId)
            .map(copy),
        group: async (options) => {
            groupCalls.push(copy(options));
            const tabIds = Array.isArray(options.tabIds) ? options.tabIds : [options.tabIds];
            let groupId = options.groupId;
            if (!Number.isInteger(groupId)) {
                groupId = nextGroupId++;
                const firstTab = tabs.get(tabIds[0]);
                const group = {
                    id: groupId,
                    title: undefined,
                    color: "grey",
                    shared: false,
                    windowId: options.createProperties?.windowId ?? firstTab.windowId,
                };
                groups.set(groupId, group);
                groupCreated.dispatch(copy(group));
            }
            for (const tabId of tabIds) tabs.get(tabId).groupId = groupId;
            return groupId;
        },
        move: async (tabId, properties) => {
            tabMoves.push({ tabId, properties: copy(properties) });
            tabs.get(tabId).windowId = properties.windowId;
            return copy(tabs.get(tabId));
        },
        update: async (tabId, properties) => {
            Object.assign(tabs.get(tabId), properties);
            return copy(tabs.get(tabId));
        },
        ungroup: async (tabIds) => {
            for (const tabId of Array.isArray(tabIds) ? tabIds : [tabIds]) {
                tabs.get(tabId).groupId = -1;
            }
        },
    },
    tabGroups: {
        TAB_GROUP_ID_NONE: -1,
        onMoved: event(),
        onCreated: groupCreated,
        onUpdated: groupUpdated,
        onRemoved: groupRemoved,
        get: async (groupId) => {
            const group = groups.get(groupId);
            if (!group) throw new Error("Group not found");
            return copy(group);
        },
        query: async () => [...groups.values()].map(copy),
        update: async (groupId, properties) => {
            const group = groups.get(groupId);
            if (!group) throw new Error("Group not found");
            Object.assign(group, properties);
            groupUpdates.push({ groupId, properties: copy(properties) });
            groupUpdated.dispatch(copy(group));
            return copy(group);
        },
    },
    windows: {
        get: async (windowId) => ({ id: windowId, type: "normal", incognito: false }),
        update: async (windowId, properties) => {
            windowUpdates.push({ windowId, properties: copy(properties) });
        },
    },
};

await import("./service_worker.js");

const savedGroup = (id = "A") => ({
    id,
    title: id,
    color: "blue",
    pages: [{ title: "Example", url: "https://example.com/docs", updatedAt: 1 }],
    customRules: [{ id: "rule-a", pattern: "https://example.com/*", updatedAt: 2 }],
    updatedAt: 2,
});

const reset = ({ saved = [savedGroup()], bindings = {}, liveGroups = [], liveTabs = [] } = {}) => {
    local[TAB_GROUP_CONFIG_KEY] = { enabled: true };
    local[TAB_GROUP_CACHE_KEY] = { version: 1, groups: copy(saved) };
    session[TAB_GROUP_BINDINGS_KEY] = copy(bindings);
    groups.clear();
    tabs.clear();
    groupCalls.length = 0;
    groupUpdates.length = 0;
    tabMoves.length = 0;
    windowUpdates.length = 0;
    nextGroupId = 100;
    for (const group of liveGroups) groups.set(group.id, copy(group));
    for (const tab of liveTabs) tabs.set(tab.id, copy(tab));
};

const dispatchTabNavigation = (tabId) => {
    const tab = copy(tabs.get(tabId));
    tabUpdated.dispatch(tabId, { url: tab.url }, tab);
};

const waitFor = async (predicate) => {
    for (let attempt = 0; attempt < 100; attempt += 1) {
        if (predicate()) return;
        await new Promise((resolve) => setImmediate(resolve));
    }
    throw new Error("Timed out waiting for service-worker operation");
};

const refresh = () => new Promise((resolve, reject) => {
    const listener = runtimeMessage.listeners[0];
    listener({ type: "tab-groups:refresh" }, {}, (response) => {
        if (response.ok) resolve(response.result);
        else reject(new Error(response.error));
    });
});

const normalGroup = (id, title, windowId = 1) => ({
    id,
    title,
    color: "blue",
    shared: false,
    windowId,
});

const ungroupedTab = (id) => ({
    id,
    groupId: -1,
    windowId: 1,
    url: "https://example.com/docs",
    title: "Example",
    active: true,
    pinned: false,
    incognito: false,
});

test("missing session binding reuses the matching live group", async () => {
    reset({ liveGroups: [normalGroup(5, "A", 2)], liveTabs: [ungroupedTab(10)] });

    dispatchTabNavigation(10);
    await waitFor(() => session[TAB_GROUP_BINDINGS_KEY]["5"] === "A");

    assert.deepEqual(groupCalls[0], { tabIds: 10, groupId: 5 });
    assert.deepEqual(tabMoves, [{ tabId: 10, properties: { windowId: 2, index: -1 } }]);
    assert.deepEqual(windowUpdates, [{ windowId: 2, properties: { focused: true } }]);
    assert.equal(local[TAB_GROUP_CACHE_KEY].groups[0].customRules.length, 1);
});

test("dead session binding is discarded before recovering by name", async () => {
    reset({
        bindings: { 99: "A" },
        liveGroups: [normalGroup(5, "A")],
        liveTabs: [ungroupedTab(13)],
    });

    dispatchTabNavigation(13);
    await waitFor(() => session[TAB_GROUP_BINDINGS_KEY]["5"] === "A");

    assert.equal("99" in session[TAB_GROUP_BINDINGS_KEY], false);
    assert.deepEqual(groupCalls[0], { tabIds: 13, groupId: 5 });
});

test("new group creation never migrates rules through its temporary empty title", async () => {
    reset({ liveTabs: [ungroupedTab(11)] });

    dispatchTabNavigation(11);
    await waitFor(() => groupUpdates.length === 1);
    await refresh();

    assert.deepEqual(groupCalls[0], {
        tabIds: 11,
        createProperties: { windowId: 1 },
    });
    assert.deepEqual(groupUpdates[0], {
        groupId: 100,
        properties: { title: "A", color: "blue" },
    });
    assert.equal(session[TAB_GROUP_BINDINGS_KEY]["100"], "A");
    assert.deepEqual(local[TAB_GROUP_CACHE_KEY].groups.map((group) => group.id), ["A"]);
    assert.equal(local[TAB_GROUP_CACHE_KEY].groups[0].customRules[0].id, "rule-a");
});

test("a mismatched binding cannot move A rules while another live A exists", async () => {
    reset({
        bindings: { 20: "A" },
        liveGroups: [normalGroup(20, "B"), normalGroup(21, "A")],
        liveTabs: [ungroupedTab(12)],
    });

    dispatchTabNavigation(12);
    await waitFor(() => groupCalls.length === 1);

    assert.deepEqual(groupCalls[0], { tabIds: 12, groupId: 21 });
    assert.equal(session[TAB_GROUP_BINDINGS_KEY]["20"], "B");
    assert.equal(session[TAB_GROUP_BINDINGS_KEY]["21"], "A");
    const cacheGroups = local[TAB_GROUP_CACHE_KEY].groups;
    assert.equal(cacheGroups.find((group) => group.id === "A").customRules.length, 1);
    assert.equal(cacheGroups.find((group) => group.id === "B").customRules.length, 0);
});

test("a sole named live group still migrates its rules on a real rename", async () => {
    reset({ bindings: { 30: "A" }, liveGroups: [normalGroup(30, "B")] });

    groupUpdated.dispatch(normalGroup(30, "B"));
    await waitFor(() => local[TAB_GROUP_CACHE_KEY].groups.some((group) => group.id === "B"));

    assert.equal(local[TAB_GROUP_CACHE_KEY].groups.some((group) => group.id === "A"), false);
    assert.equal(local[TAB_GROUP_CACHE_KEY].groups[0].customRules[0].id, "rule-a");
    assert.equal(session[TAB_GROUP_BINDINGS_KEY]["30"], "B");
});

test("unnamed groups neither persist nor take ownership of saved rules", async () => {
    reset({ bindings: { 40: "A" }, liveGroups: [normalGroup(40, undefined)] });

    groupCreated.dispatch(normalGroup(40, undefined));
    await waitFor(() => !("40" in session[TAB_GROUP_BINDINGS_KEY]));

    assert.deepEqual(local[TAB_GROUP_CACHE_KEY].groups.map((group) => group.id), ["A"]);
    assert.equal(local[TAB_GROUP_CACHE_KEY].groups[0].customRules[0].id, "rule-a");
});
