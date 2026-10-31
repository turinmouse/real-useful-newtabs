export const NEWTAB_POSITION_KEY = "keep_newtab_rightmost";

export function installNewtabPosition(api) {
    const pageUrl = api.runtime.getURL("index.html");
    const isNewtab = (tab) => {
        const url = (tab.pendingUrl || tab.url || "").split(/[?#]/, 1)[0];
        return !tab.pinned && !tab.incognito
            && (url === "chrome://newtab/" || url === "chrome://newtab" || url === pageUrl);
    };
    let queue = Promise.resolve();
    const reconcile = async () => {
        const stored = await api.storage.local.get(NEWTAB_POSITION_KEY);
        if (stored[NEWTAB_POSITION_KEY] !== true) return;
        const windows = new Map();
        for (const tab of await api.tabs.query({ windowType: "normal" })) {
            if (tab.incognito) continue;
            if (!windows.has(tab.windowId)) windows.set(tab.windowId, []);
            windows.get(tab.windowId).push(tab);
        }
        for (const tabs of windows.values()) {
            tabs.sort((a, b) => a.index - b.index);
            const newtabs = tabs.filter(isNewtab);
            const suffix = tabs.slice(tabs.length - newtabs.length);
            const ordered = newtabs.every((tab, i) => tab.id === suffix[i]?.id);
            const grouped = newtabs.filter((tab) => tab.groupId >= 0);
            if (ordered && grouped.length === 0) continue;
            for (const candidate of newtabs) {
                // Tabs may close, navigate or become pinned while earlier API calls run.
                try {
                    const tab = await api.tabs.get(candidate.id);
                    if (!isNewtab(tab) || tab.windowId !== candidate.windowId) continue;
                    const current = await api.storage.local.get(NEWTAB_POSITION_KEY);
                    if (current[NEWTAB_POSITION_KEY] !== true) return;
                    if (tab.groupId >= 0) await api.tabs.ungroup(tab.id);
                    await api.tabs.move(tab.id, { index: -1 });
                } catch (error) {
                    console.warn("Unable to move new tab page", error);
                }
            }
        }
    };
    const schedule = () => {
        queue = queue.then(reconcile).catch((error) => {
            console.warn("Unable to maintain new tab position", error);
        });
        return queue;
    };
    for (const event of [api.tabs.onCreated, api.tabs.onMoved, api.tabs.onAttached,
        api.tabs.onDetached, api.tabs.onRemoved, api.tabGroups.onMoved,
        api.runtime.onStartup, api.runtime.onInstalled]) {
        event.addListener(schedule);
    }
    api.tabs.onUpdated.addListener((_id, change) => {
        if (["url", "status", "pinned", "groupId"].some((key) => key in change)) schedule();
    });
    api.storage.onChanged.addListener((changes, area) => {
        if (area === "local" && changes[NEWTAB_POSITION_KEY]) schedule();
    });
    // Also recover when Chrome wakes a previously suspended service worker.
    schedule();
    return schedule;
}
