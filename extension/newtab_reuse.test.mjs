import test from "node:test";
import assert from "node:assert/strict";
import { installNewtabReuse, NEWTAB_REUSE_KEY } from "./newtab_reuse.mjs";

const event = () => {
    const listeners = [];
    return {
        addListener: (fn) => listeners.push(fn),
        fire: (...args) => Promise.all(listeners.map((fn) => fn(...args))),
    };
};
const N = "chrome://newtab/";
const tab = (id, extra = {}) => ({ id, index: id, windowId: 1, url: N, ...extra });
function fixture(initial = [], enabled = true) {
    const tabs = new Map(initial.map((item) => [item.id, item]));
    const actions = [];
    const api = {
        runtime: { getURL: (path) => `chrome-extension://test/${path}` },
        storage: { local: { get: async () => ({ [NEWTAB_REUSE_KEY]: enabled }) } },
        windows: { get: async () => ({ type: "normal", incognito: false }) },
        tabs: {
            ...Object.fromEntries(["onCreated", "onUpdated", "onRemoved", "onActivated"].map((key) => [key, event()])),
            get: async (id) => {
                if (!tabs.has(id)) throw new Error("Tab closed");
                return structuredClone(tabs.get(id));
            },
            query: async ({ windowId }) => [...tabs.values()].filter((t) => t.windowId === windowId).map((t) => structuredClone(t)),
            update: async (id, updateProperties) => { actions.push(["update", id, updateProperties]); },
            move: async (id, { index }) => {
                actions.push(["move", id, index]);
                const t = tabs.get(id);
                if (t) t.index = index;
            },
            remove: async (id) => { actions.push(["remove", id]); tabs.delete(id); },
        },
    };
    installNewtabReuse(api);
    return { api, tabs, actions, create: (t) => { tabs.set(t.id, t); return api.tabs.onCreated.fire(t); } };
}

test("reuses the most recently accessed new tab in the same window", async () => {
    const f = fixture([tab(1, { lastAccessed: 10 }), tab(2, { lastAccessed: 20 }), tab(3, { windowId: 2, lastAccessed: 30 })]);
    await f.create(tab(4));
    assert.deepEqual(f.actions, [["remove", 2], ["move", 4, 2]]);
    assert.ok(!f.tabs.has(2));
    assert.ok(f.tabs.has(4));
});

test("pinned new tabs are not reused or removed", async () => {
    const f = fixture([tab(1, { lastAccessed: 20, pinned: true }), tab(2, { lastAccessed: 10 })]);
    await f.create(tab(3));
    assert.deepEqual(f.actions, [["remove", 2], ["move", 3, 2]]);
    assert.ok(f.tabs.has(1));
    assert.ok(!f.tabs.has(2));
    assert.ok(f.tabs.has(3));
});

test("activation events supply recency on older Chrome versions", async () => {
    const f = fixture([tab(1), tab(2)]);
    await f.api.tabs.onActivated.fire({ tabId: 2 });
    await f.create(tab(3));
    assert.deepEqual(f.actions, [["remove", 2], ["move", 3, 2]]);
});

test("disabled setting, missing candidate, other windows and ordinary URLs preserve new tabs", async () => {
    for (const [existing, enabled, created] of [
        [[tab(1)], false, tab(2)],
        [[], true, tab(2)],
        [[tab(1, { windowId: 2 })], true, tab(2)],
        [[tab(1)], true, tab(2, { url: "https://example.com" })],
        [[tab(1)], true, tab(2, { incognito: true })],
        [[tab(1)], true, tab(2, { pinned: true })],
    ]) {
        const f = fixture(existing, enabled);
        await f.create(created);
        assert.deepEqual(f.actions, []);
        assert.ok(f.tabs.has(2));
    }
});

test("waits for an initially missing URL but ignores navigation of preexisting tabs", async () => {
    const f = fixture([tab(1), tab(2, { url: "https://example.com" })]);
    await f.create(tab(3, { url: "" }));
    assert.deepEqual(f.actions, []);
    f.tabs.get(3).pendingUrl = "chrome-extension://test/index.html#canvas";
    await f.api.tabs.onUpdated.fire(3);
    assert.deepEqual(f.actions, [["remove", 1], ["move", 3, 1]]);
    f.tabs.get(2).url = N;
    await f.api.tabs.onUpdated.fire(2);
    assert.ok(f.tabs.has(2));
});

test("rapid creation keeps one new tab instead of closing every candidate", async () => {
    const f = fixture();
    await Promise.all([f.create(tab(1)), f.create(tab(2)), f.create(tab(3))]);
    assert.deepEqual([...f.tabs.keys()], [3]);
    assert.deepEqual(f.actions, [["remove", 1], ["move", 2, 1], ["remove", 2], ["move", 3, 1]]);
});

test("navigation away during lookup prevents closing the existing tab", async () => {
    const f = fixture([tab(1)]);
    const get = f.api.tabs.get;
    f.api.tabs.get = async (id) => {
        const t = await get(id);
        if (id === 2) t.pendingUrl = "https://example.com";
        return t;
    };
    await f.create(tab(2));
    assert.ok(f.tabs.has(1));
    assert.ok(f.tabs.has(2));
    assert.deepEqual(f.actions, []);
});

test("a candidate that closes during lookup is skipped", async () => {
    const f = fixture([tab(1, { lastAccessed: 20 }), tab(2, { lastAccessed: 10 })]);
    const get = f.api.tabs.get;
    f.api.tabs.get = async (id) => {
        if (id === 1) f.tabs.delete(1);
        return get(id);
    };
    await f.create(tab(3));
    assert.deepEqual(f.actions, [["remove", 2], ["move", 3, 2]]);
});

test("existing duplicates are untouched on installation and popup windows are excluded", async () => {
    const f = fixture([tab(1), tab(2)]);
    assert.deepEqual(f.actions, []);
    f.api.windows.get = async () => ({ type: "popup" });
    await f.create(tab(3));
    assert.deepEqual(f.actions, []);
});
