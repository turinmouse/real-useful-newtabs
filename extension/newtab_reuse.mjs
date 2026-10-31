export const NEWTAB_REUSE_KEY = "reuse_newtab";

export function installNewtabReuse(api) {
    const pageUrl = api.runtime.getURL("index.html");
    const urlOf = (tab) => (tab.pendingUrl || tab.url || "").split(/[?#]/, 1)[0];
    const isNewtab = (tab) => !tab.incognito
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
            if (!isNewtab(created) || created.pinned) return;
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
                    || current.pinned || current.windowId !== target.windowId) return;
                await api.tabs.update(target.id, { active: true });
                // Recheck before closing: navigation may have started during activation.
                const closing = await api.tabs.get(id);
                if (isNewtab(closing) && !closing.pinned && closing.windowId === target.windowId) {
                    await api.tabs.remove(id);
                }
                return;
            }
        } catch (error) {
            pending.delete(id);
            // Preserve the new page if its replacement cannot be activated.
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
        if (!Number.isInteger(tab.id) || tab.incognito) return;
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
