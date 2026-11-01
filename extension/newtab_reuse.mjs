export const NEWTAB_REUSE_KEY = "reuse_newtab";

export function installNewtabReuse(api) {
    const pageUrl = api.runtime.getURL("index.html");
    const urlOf = (tab) => (tab.pendingUrl || tab.url || "").split(/[?#]/, 1)[0];
    const isNewtab = (tab) => !tab.incognito && !tab.pinned
        && ["chrome://newtab/", "chrome://newtab", pageUrl].includes(urlOf(tab));
    const pending = new Set();
    const accessed = new Map();
    let queue = Promise.resolve();

    const reuse = async (id) => {
        if (!pending.has(id)) return;
        try {
            const config = await api.storage.local.get(NEWTAB_REUSE_KEY);
            if (config[NEWTAB_REUSE_KEY] !== true) {
                pending.delete(id);
                return;
            }
            const created = await api.tabs.get(id);
            // Chrome can omit both URLs in onCreated. Wait for its initial URL update.
            if (!urlOf(created)) return;
            pending.delete(id);
            if (!isNewtab(created)) return;
            const window = await api.windows.get(created.windowId);
            if (window.type !== "normal" || window.incognito) return;
            const recency = (tab) => Math.max(tab.lastAccessed || 0, accessed.get(tab.id) || 0);
            const candidates = (await api.tabs.query({ windowId: created.windowId }))
                .filter((tab) => tab.id !== id && !pending.has(tab.id) && isNewtab(tab))
                .sort((a, b) => recency(b) - recency(a) || a.index - b.index);
            for (const candidate of candidates) {
                let target;
                try {
                    target = await api.tabs.get(candidate.id);
                } catch {
                    continue;
                }
                if (!isNewtab(target) || target.windowId !== created.windowId) continue;
                const current = await api.tabs.get(id);
                const latest = await api.storage.local.get(NEWTAB_REUSE_KEY);
                if (latest[NEWTAB_REUSE_KEY] !== true || !isNewtab(current)
                    || current.windowId !== target.windowId) return;
                const targetIndex = target.index;
                await api.tabs.remove(target.id);
                try {
                    const remaining = await api.tabs.get(id);
                    if (isNewtab(remaining) && remaining.windowId === target.windowId
                        && remaining.index !== targetIndex && api.tabs.move) {
                        await api.tabs.move(id, { index: targetIndex });
                    }
                } catch {
                    // Tab may have been closed or navigated concurrently.
                }
                return;
            }
        } catch (error) {
            pending.delete(id);
            console.warn("Unable to reuse new tab page", error);
        }
    };
    const schedule = (id) => {
        queue = queue.then(() => reuse(id)).catch((error) => {
            console.warn("Unable to process new tab reuse", error);
        });
        return queue;
    };
    api.tabs.onCreated.addListener((tab) => {
        if (!Number.isInteger(tab.id) || tab.incognito || tab.pinned) return;
        pending.add(tab.id);
        return schedule(tab.id);
    });
    api.tabs.onUpdated.addListener((id) => {
        if (pending.has(id)) return schedule(id);
    });
    api.tabs.onRemoved.addListener((id) => {
        pending.delete(id);
        accessed.delete(id);
    });
    api.tabs.onActivated.addListener(({ tabId }) => accessed.set(tabId, Date.now()));
}
