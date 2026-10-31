export const TAB_GROUP_CACHE_KEY = "tab_group_cache_v1";
export const TAB_GROUP_CONFIG_KEY = "tab_group_config_v1";
export const TAB_GROUP_BINDINGS_KEY = "tab_group_bindings_v1";
export const TAB_GROUP_PAGE_MATCH_LEVEL = "full_path";

export const DEFAULT_TAB_GROUP_CONFIG = Object.freeze({
    enabled: false,
});

export const normalizeConfig = (value) => ({
    enabled: value?.enabled === true,
});

export const normalizeUrl = (value, matchLevel) => {
    let url;
    try {
        url = new URL(value);
    } catch {
        return null;
    }
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;

    switch (matchLevel) {
        case "domain":
            return url.hostname.toLowerCase();
        case "full_path":
            return `${url.origin}${url.pathname}`;
        case "query":
            return `${url.origin}${url.pathname}${url.search}`;
        case "fragment":
            return url.href;
        default:
            return `${url.origin}${url.pathname}`;
    }
};

export const isPinnableTab = (tab, noGroupId) => Number.isInteger(tab?.id)
    && Number.isInteger(tab?.groupId)
    && tab.groupId !== noGroupId
    && !tab.incognito
    && !tab.pinned
    && normalizeUrl(tab.url, "fragment") !== null;

export const domainRuleForUrl = (value) => {
    let url;
    try {
        url = new URL(value);
    } catch {
        return null;
    }
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return `*://${url.host}/*`;
};

export const pinDomainRule = (
    cache,
    groupId,
    value,
    ruleId,
    now = Date.now(),
) => {
    const group = findGroup(cache, groupId);
    const pattern = domainRuleForUrl(value);
    if (!group || !pattern) return null;

    const existing = group.customRules.find((rule) => rule.pattern === pattern);
    if (existing) return null;

    const rule = addCustomRule(cache, groupId, ruleId, pattern, now);
    return rule ? { pattern } : null;
};

const normalizedPage = (page) => ({
    title: typeof page.title === "string" ? page.title : "",
    url: page.url,
    updatedAt: Number.isSafeInteger(page.updatedAt) ? page.updatedAt : 0,
});

export const normalizeCustomRulePattern = (value) => {
    if (typeof value !== "string") return null;
    const pattern = value.trim();
    if (!pattern) return null;

    let escaping = false;
    for (const character of pattern) {
        if (escaping) {
            escaping = false;
        } else if (character === "\\") {
            escaping = true;
        }
    }
    return escaping ? null : pattern;
};

const normalizedCustomRule = (rule) => {
    const pattern = normalizeCustomRulePattern(rule?.pattern);
    if (!pattern) return null;
    const updatedAt = Number.isSafeInteger(rule?.updatedAt) ? rule.updatedAt : 0;
    return {
        id: typeof rule?.id === "string" && rule.id
            ? rule.id
            : `legacy:${pattern}`,
        pattern,
        updatedAt,
    };
};

const mergePages = (target, pages) => {
    for (const page of pages) {
        const existing = target.find((candidate) => candidate.url === page.url);
        if (!existing) {
            target.push(page);
        } else if (page.updatedAt >= existing.updatedAt) {
            Object.assign(existing, page);
        }
    }
};

const mergeCustomRules = (target, rules) => {
    for (const rule of rules) {
        const existing = target.find((candidate) => candidate.pattern === rule.pattern);
        if (!existing) {
            target.push(rule);
        } else if (rule.updatedAt >= existing.updatedAt) {
            Object.assign(existing, rule);
        }
    }
};

export const normalizeCache = (value) => {
    const groups = [];
    for (const raw of Array.isArray(value?.groups) ? value.groups : []) {
        const title = typeof raw?.title === "string" ? raw.title : "";
        const updatedAt = Number.isSafeInteger(raw?.updatedAt) ? raw.updatedAt : 0;
        const pages = (Array.isArray(raw?.pages) ? raw.pages : [])
            .filter((page) => typeof page?.url === "string")
            .map(normalizedPage);
        const customRules = (Array.isArray(raw?.customRules) ? raw.customRules : [])
            .map(normalizedCustomRule)
            .filter(Boolean);
        let saved = groups.find((group) => group.id === title);
        if (!saved) {
            saved = {
                id: title,
                title,
                color: typeof raw?.color === "string" ? raw.color : "grey",
                pages: [],
                customRules: [],
                updatedAt,
            };
            groups.push(saved);
        } else if (updatedAt >= saved.updatedAt) {
            saved.color = typeof raw?.color === "string" ? raw.color : "grey";
            saved.updatedAt = updatedAt;
        }
        mergePages(saved.pages, pages);
        mergeCustomRules(saved.customRules, customRules);
    }
    return { version: 1, groups };
};

export const findGroup = (cache, id) => cache.groups.find((group) => group.id === id) ?? null;

export const findLiveGroupByName = (groups, logicalId) => {
    if (typeof logicalId !== "string" || !logicalId) return null;
    let match = null;
    for (const group of Array.isArray(groups) ? groups : []) {
        if (
            group?.shared
            || !Number.isInteger(group?.id)
            || group?.title !== logicalId
        ) {
            continue;
        }
        if (!match || group.id < match.id) match = group;
    }
    return match;
};

export const findGroupForUrl = (cache, url, matchLevel) => {
    const key = normalizeUrl(url, matchLevel);
    if (!key) return null;
    let best = null;
    for (const group of cache.groups) {
        if (!group.id) continue;
        for (const page of group.pages) {
            if (normalizeUrl(page.url, matchLevel) !== key) continue;
            if (!best || page.updatedAt > best.updatedAt) {
                best = { group, updatedAt: page.updatedAt };
            }
        }
    }
    if (best) return best.group;

    let fullUrl;
    try {
        const parsed = new URL(url);
        if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return null;
        fullUrl = parsed.href;
    } catch {
        return null;
    }

    let bestRule = null;
    for (const group of cache.groups) {
        if (!group.id) continue;
        for (const rule of group.customRules) {
            if (!customRuleMatchesUrl(rule.pattern, fullUrl)) continue;
            if (
                !bestRule
                || rule.updatedAt > bestRule.rule.updatedAt
                || (
                    rule.updatedAt === bestRule.rule.updatedAt
                    && `${rule.id}\0${group.id}` > `${bestRule.rule.id}\0${bestRule.group.id}`
                )
            ) {
                bestRule = { group, rule };
            }
        }
    }
    return bestRule?.group ?? null;
};

export const customRuleMatchesUrl = (pattern, value) => {
    const normalized = normalizeCustomRulePattern(pattern);
    if (!normalized) return false;
    let expression = "^";
    let escaping = false;
    for (const character of normalized) {
        if (escaping) {
            expression += character.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
            escaping = false;
        } else if (character === "\\") {
            escaping = true;
        } else if (character === "*") {
            expression += ".*";
        } else if (character === "?") {
            expression += ".";
        } else {
            expression += character.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        }
    }
    try {
        return new RegExp(`${expression}$`, "u").test(value);
    } catch {
        return false;
    }
};

export const upsertGroup = (cache, group, now = Date.now()) => {
    const name = typeof group.title === "string" ? group.title : "";
    let saved = findGroup(cache, name);
    if (!saved) {
        saved = {
            id: name,
            title: name,
            color: "grey",
            pages: [],
            customRules: [],
            updatedAt: now,
        };
        cache.groups.push(saved);
    }
    saved.id = name;
    saved.title = name;
    saved.color = typeof group.color === "string" ? group.color : "grey";
    saved.updatedAt = now;
    return saved;
};

export const renameGroup = (cache, id, title, now = Date.now()) => {
    const source = findGroup(cache, id);
    if (!source) return null;
    if (id === title) {
        source.title = title;
        source.updatedAt = now;
        return source;
    }

    const existing = findGroup(cache, title);
    if (existing) {
        mergePages(existing.pages, source.pages);
        mergeCustomRules(existing.customRules, source.customRules);
        existing.updatedAt = now;
        cache.groups = cache.groups.filter((group) => group !== source);
        return existing;
    }

    source.id = title;
    source.title = title;
    source.updatedAt = now;
    return source;
};

export const addCustomRule = (cache, groupId, id, pattern, now = Date.now()) => {
    const group = findGroup(cache, groupId);
    const normalized = normalizeCustomRulePattern(pattern);
    if (!group || !normalized || typeof id !== "string" || !id) return null;
    const existing = group.customRules.find((rule) => rule.pattern === normalized);
    if (existing) {
        existing.updatedAt = now;
        group.updatedAt = now;
        return existing;
    }
    const rule = { id, pattern: normalized, updatedAt: now };
    group.customRules.push(rule);
    group.updatedAt = now;
    return rule;
};

export const updateCustomRule = (
    cache,
    groupId,
    ruleId,
    pattern,
    now = Date.now(),
) => {
    const group = findGroup(cache, groupId);
    const normalized = normalizeCustomRulePattern(pattern);
    const rule = group?.customRules.find((candidate) => candidate.id === ruleId);
    if (!group || !normalized || !rule) return null;
    const duplicate = group.customRules.find(
        (candidate) => candidate.id !== ruleId && candidate.pattern === normalized,
    );
    if (duplicate) {
        duplicate.updatedAt = now;
        group.customRules = group.customRules.filter((candidate) => candidate.id !== ruleId);
        group.updatedAt = now;
        return duplicate;
    }
    rule.pattern = normalized;
    rule.updatedAt = now;
    group.updatedAt = now;
    return rule;
};

export const deleteCustomRule = (cache, groupId, ruleId) => {
    const group = findGroup(cache, groupId);
    if (!group) return false;
    const previousLength = group.customRules.length;
    group.customRules = group.customRules.filter((rule) => rule.id !== ruleId);
    return group.customRules.length !== previousLength;
};

export const recordPage = (cache, logicalId, tab, matchLevel, now = Date.now()) => {
    const key = normalizeUrl(tab.url, matchLevel);
    const group = findGroup(cache, logicalId);
    if (!key || !group) return false;

    for (const candidate of cache.groups) {
        if (candidate.id === logicalId) continue;
        candidate.pages = candidate.pages.filter(
            (page) => normalizeUrl(page.url, matchLevel) !== key,
        );
    }

    const existing = group.pages.find((page) => page.url === tab.url);
    if (existing) {
        existing.title = typeof tab.title === "string" ? tab.title : existing.title;
        existing.updatedAt = now;
    } else {
        group.pages.push({
            title: typeof tab.title === "string" ? tab.title : "",
            url: tab.url,
            updatedAt: now,
        });
    }
    group.updatedAt = now;
    return true;
};

export const pinPageMapping = (
    cache,
    groupId,
    tab,
    matchLevel = TAB_GROUP_PAGE_MATCH_LEVEL,
    now = Date.now(),
) => {
    const key = normalizeUrl(tab?.url, matchLevel);
    const group = findGroup(cache, groupId);
    if (!key || !group) return null;

    const existing = group.pages.some(
        (page) => normalizeUrl(page.url, matchLevel) === key,
    );
    if (existing) return null;

    return recordPage(cache, groupId, tab, matchLevel, now)
        ? { key }
        : null;
};

export const removeUrlMapping = (cache, url, matchLevel) => {
    const key = normalizeUrl(url, matchLevel);
    if (!key) return false;
    let changed = false;
    for (const group of cache.groups) {
        const previousLength = group.pages.length;
        group.pages = group.pages.filter(
            (page) => normalizeUrl(page.url, matchLevel) !== key,
        );
        changed ||= group.pages.length !== previousLength;
    }
    return changed;
};

export const deletePageMapping = (cache, groupId, url, matchLevel) => {
    const key = normalizeUrl(url, matchLevel);
    const group = findGroup(cache, groupId);
    if (!key || !group) return false;
    if (!group.pages.some((page) => normalizeUrl(page.url, matchLevel) === key)) return false;
    return removeUrlMapping(cache, url, matchLevel);
};

export const reindexCache = (cache, matchLevel) => {
    const winners = new Map();
    for (const group of cache.groups) {
        for (const page of group.pages) {
            const key = normalizeUrl(page.url, matchLevel);
            if (!key) continue;
            const winner = winners.get(key);
            if (!winner || page.updatedAt > winner.page.updatedAt) {
                winners.set(key, { groupId: group.id, page });
            }
        }
    }
    for (const group of cache.groups) {
        group.pages = group.pages.filter((page) => {
            const key = normalizeUrl(page.url, matchLevel);
            return key && winners.get(key)?.groupId === group.id;
        });
    }
    return cache;
};

export const deleteGroup = (cache, id) => {
    const previousLength = cache.groups.length;
    cache.groups = cache.groups.filter((group) => group.id !== id);
    return cache.groups.length !== previousLength;
};
