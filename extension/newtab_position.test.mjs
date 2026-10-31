import test from "node:test";
import assert from "node:assert/strict";
import { installNewtabPosition, NEWTAB_POSITION_KEY } from "./newtab_position.mjs";

const event = () => {
    const listeners = [];
    return {
        addListener: (fn) => listeners.push(fn),
        fire: (...args) => Promise.all(listeners.map((fn) => fn(...args))),
    };
};
const fixture = (urls, enabled = true) => {
    const tabs = urls.map((url, id) => ({ id, url, index: id, windowId: 1, groupId: -1 }));
    const moves = [];
    const api = {
        runtime: { getURL: (path) => `chrome-extension://test/${path}`, onStartup: event(), onInstalled: event() },
        storage: {
            local: { get: async () => ({ [NEWTAB_POSITION_KEY]: enabled }) },
            onChanged: event(),
        },
        tabGroups: { onMoved: event() },
        tabs: {
            ...Object.fromEntries(["onCreated", "onUpdated", "onMoved", "onAttached", "onDetached", "onRemoved"].map((key) => [key, event()])),
            query: async () => structuredClone(tabs),
            get: async (id) => structuredClone(tabs.find((tab) => tab.id === id)),
            ungroup: async (id) => { tabs.find((tab) => tab.id === id).groupId = -1; },
            move: async (id) => {
                moves.push(id);
                const tab = tabs.find((tab) => tab.id === id);
                for (const other of tabs) {
                    if (other.windowId === tab.windowId && other.index > tab.index) other.index--;
                }
                tab.index = tabs.filter((other) => other.windowId === tab.windowId).length - 1;
                void api.tabs.onMoved.fire(id, { windowId: tab.windowId });
            },
        },
    };
    const flush = installNewtabPosition(api);
    return { tabs, moves, api, flush, setEnabled: (value) => { enabled = value; } };
};
const N = "chrome://newtab/";
const A = "https://example.com/";

test("disabled by default; enabling reconciles existing tabs and disabling stops moves", async () => {
    const f = fixture([N, A], undefined);
    f.setEnabled(undefined);
    await f.flush();
    assert.deepEqual(f.moves, []);
    f.setEnabled(true);
    await f.api.storage.onChanged.fire({ [NEWTAB_POSITION_KEY]: { newValue: true } }, "local");
    await f.flush();
    assert.deepEqual(f.moves, [0]);
    f.setEnabled(false);
    f.tabs[0].index = 0;
    f.tabs[1].index = 1;
    await f.api.tabs.onMoved.fire(0, { windowId: 1 });
    assert.deepEqual(f.moves, [0]);
});

test("multiple new tabs form a stable suffix without move event loops", async () => {
    const f = fixture([N, A, "chrome-extension://test/index.html#canvas", A, N]);
    await f.flush();
    const order = [...f.tabs].sort((a, b) => a.index - b.index).map((tab) => tab.id);
    assert.deepEqual(order, [1, 3, 0, 2, 4]);
    const count = f.moves.length;
    await f.flush();
    assert.equal(f.moves.length, count);
    f.tabs.push({ id: 5, index: 5, url: A, windowId: 1, groupId: -1 });
    await f.api.tabs.onCreated.fire(f.tabs.at(-1));
    await f.flush();
    assert.equal(f.tabs[4].index, 5);
});

test("pinned and incognito pages stay put; pending navigation away releases position", async () => {
    const f = fixture([N, N, N, A]);
    f.tabs[0].pinned = true;
    f.tabs[1].incognito = true;
    f.tabs[2].pendingUrl = A;
    await f.flush();
    assert.deepEqual(f.moves, []);
});

test("pending new tabs and grouped pages move within their own windows", async () => {
    const f = fixture(["", A, N, A]);
    f.tabs[0].pendingUrl = N;
    f.tabs[0].groupId = 10;
    f.tabs[2].windowId = f.tabs[3].windowId = 2;
    f.tabs[2].index = 0;
    f.tabs[3].index = 1;
    await f.api.tabs.onAttached.fire(2, { newWindowId: 2 });
    await f.flush();
    assert.equal(f.tabs[0].groupId, -1);
    assert.equal(f.tabs[0].index, 1);
    assert.equal(f.tabs[2].index, 1);
    assert.equal(f.tabs[2].windowId, 2);
});
