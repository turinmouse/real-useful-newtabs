import test from "node:test";
import assert from "node:assert/strict";

import {
    DEFAULT_TAB_GROUP_CONFIG,
    TAB_GROUP_PAGE_MATCH_LEVEL,
    addCustomRule,
    customRuleMatchesUrl,
    deleteCustomRule,
    deletePageMapping,
    domainRuleForUrl,
    findGroupForUrl,
    findLiveGroupByName,
    isPinnableTab,
    normalizeCache,
    normalizeConfig,
    normalizeUrl,
    pinDomainRule,
    pinPageMapping,
    recordPage,
    renameGroup,
    reindexCache,
    removeUrlMapping,
    updateCustomRule,
} from "./tab_group_rules.mjs";

const group = (id, pages = []) => ({
    id,
    title: id,
    color: "blue",
    pages,
    updatedAt: 0,
});

const page = (url, updatedAt) => ({ title: url, url, updatedAt });
const rule = (id, pattern, updatedAt) => ({ id, pattern, updatedAt });

test("URL matching levels include exactly the configured components", () => {
    const url = "https://Sub.Example.com:8443/a/b?x=1#part";
    assert.equal(normalizeUrl(url, "domain"), "sub.example.com");
    assert.equal(normalizeUrl(url, "full_path"), "https://sub.example.com:8443/a/b");
    assert.equal(normalizeUrl(url, "query"), "https://sub.example.com:8443/a/b?x=1");
    assert.equal(normalizeUrl(url, "fragment"), "https://sub.example.com:8443/a/b?x=1#part");
});

test("URL matching rejects non-HTTP pages", () => {
    assert.equal(normalizeUrl("chrome://settings", "fragment"), null);
    assert.equal(normalizeUrl("file:///tmp/example", "fragment"), null);
    assert.equal(normalizeUrl("not a URL", "fragment"), null);
});

test("configuration contains only the enhancement toggle", () => {
    assert.deepEqual(normalizeConfig(null), DEFAULT_TAB_GROUP_CONFIG);
    assert.deepEqual(normalizeConfig({ enabled: true, matchLevel: "domain" }), { enabled: true });
    assert.equal(TAB_GROUP_PAGE_MATCH_LEVEL, "full_path");
});

test("fixed full-path page matching ignores query parameters and fragments", () => {
    const first = normalizeUrl("https://example.com/a?x=1#first", TAB_GROUP_PAGE_MATCH_LEVEL);
    const second = normalizeUrl("https://example.com/a?x=2#second", TAB_GROUP_PAGE_MATCH_LEVEL);
    assert.equal(first, "https://example.com/a");
    assert.equal(second, first);
});

test("domain pinning creates an exact-host cross-protocol custom rule", () => {
    const pattern = domainRuleForUrl("https://Sub.Example.com:8443/docs/page?q=1");
    assert.equal(pattern, "*://sub.example.com:8443/*");
    assert.equal(customRuleMatchesUrl(pattern, "http://sub.example.com:8443/other"), true);
    assert.equal(customRuleMatchesUrl(pattern, "https://child.sub.example.com:8443/other"), false);
    assert.equal(customRuleMatchesUrl(pattern, "https://sub.example.com/other"), false);
    assert.equal(domainRuleForUrl("chrome://settings"), null);
});

test("domain pinning ignores an exact rule already in the current group", () => {
    const cache = normalizeCache({
        groups: [
            { ...group("Work"), customRules: [] },
            {
                ...group("Other"),
                customRules: [rule("other", "*://example.com/*", 1)],
            },
        ],
    });
    assert.deepEqual(
        pinDomainRule(cache, "Work", "https://example.com/docs", "work", 10),
        { pattern: "*://example.com/*" },
    );
    assert.equal(cache.groups[0].customRules.length, 1);
    assert.equal(cache.groups[1].customRules.length, 1);

    assert.equal(
        pinDomainRule(cache, "Work", "http://example.com/other", "unused", 20),
        null,
    );
    assert.equal(cache.groups[0].customRules.length, 1);
    assert.equal(cache.groups[0].customRules[0].updatedAt, 10);
    assert.equal(cache.groups[1].customRules.length, 1);
});

test("only supported grouped tabs are eligible for explicit pinning", () => {
    const tab = { id: 1, groupId: 7, url: "https://example.com", pinned: false, incognito: false };
    assert.equal(isPinnableTab(tab, -1), true);
    assert.equal(isPinnableTab({ ...tab, groupId: -1 }, -1), false);
    assert.equal(isPinnableTab({ ...tab, pinned: true }, -1), false);
    assert.equal(isPinnableTab({ ...tab, incognito: true }, -1), false);
    assert.equal(isPinnableTab({ ...tab, url: "chrome://settings" }, -1), false);
});

test("cache identity is migrated to the Chrome group name", () => {
    const cache = normalizeCache({
        groups: [{
            id: "old-random-id",
            title: "Work",
            color: "blue",
            pages: [page("https://example.com", 1)],
            updatedAt: 1,
        }],
    });
    assert.equal(cache.groups[0].id, "Work");
});

test("groups with the same name merge into one cache entry", () => {
    const cache = normalizeCache({
        groups: [
            { ...group("Work", [page("https://example.com/a", 1)]), id: "first" },
            { ...group("Work", [page("https://example.com/b", 2)]), id: "second" },
        ],
    });
    assert.equal(cache.groups.length, 1);
    assert.equal(cache.groups[0].id, "Work");
    assert.deepEqual(
        cache.groups[0].pages.map(({ url }) => url),
        ["https://example.com/a", "https://example.com/b"],
    );
});

test("live groups can recover a missing session binding by persistent name", () => {
    const groups = [
        { id: 12, title: "Work", shared: false },
        { id: 7, title: "Work", shared: false },
        { id: 3, title: "Work", shared: true },
        { id: 1, title: "Other", shared: false },
    ];
    assert.equal(findLiveGroupByName(groups, "Work")?.id, 7);
    assert.equal(findLiveGroupByName(groups, "Missing"), null);
    assert.equal(findLiveGroupByName([{ id: 2, title: "", shared: false }], ""), null);
});

test("older caches default to an empty custom rule list", () => {
    const cache = normalizeCache({ groups: [group("Work")] });
    assert.deepEqual(cache.groups[0].customRules, []);
});

test("custom rules match the complete canonical URL with glob escaping", () => {
    assert.equal(
        customRuleMatchesUrl("https://*.example.com/docs/*", "https://sub.example.com/docs/a?q=1"),
        true,
    );
    assert.equal(
        customRuleMatchesUrl(String.raw`https://example.com/search\?q=te?t`, "https://example.com/search?q=test"),
        true,
    );
    assert.equal(
        customRuleMatchesUrl("https://example.com/docs", "https://example.com/docs/more"),
        false,
    );
    assert.equal(customRuleMatchesUrl("https://example.com/*\\", "https://example.com/a"), false);
});

test("explicitly pinned page ownership has priority over custom rules", () => {
    const cache = normalizeCache({
        groups: [
            { ...group("rule"), customRules: [rule("rule-1", "https://example.com/*", 20)] },
            group("recorded", [page("https://example.com/docs", 1)]),
        ],
    });
    assert.equal(findGroupForUrl(cache, "https://example.com/docs", "full_path")?.id, "recorded");
});

test("unnamed saved records never claim URL ownership", () => {
    const cache = normalizeCache({
        groups: [{
            ...group("", [page("https://example.com/docs", 10)]),
            customRules: [rule("unnamed", "https://example.com/*", 20)],
        }],
    });
    assert.equal(findGroupForUrl(cache, "https://example.com/docs", "full_path"), null);
    assert.equal(findGroupForUrl(cache, "https://example.com/other", "full_path"), null);
});

test("most recently updated custom rule wins when no page is recorded", () => {
    const cache = normalizeCache({
        groups: [
            { ...group("older"), customRules: [rule("a", "https://example.com/*", 10)] },
            { ...group("newer"), customRules: [rule("b", "https://example.com/docs/*", 20)] },
        ],
    });
    assert.equal(findGroupForUrl(cache, "https://example.com/docs/a", "full_path")?.id, "newer");
});

test("custom rules can be added, edited, deduplicated, and deleted", () => {
    const cache = normalizeCache({ groups: [group("Work")] });
    assert.equal(addCustomRule(cache, "Work", "one", " https://example.com/* ", 10)?.pattern, "https://example.com/*");
    assert.equal(addCustomRule(cache, "Work", "two", "https://example.com/*", 20)?.id, "one");
    assert.equal(updateCustomRule(cache, "Work", "one", "https://docs.example.com/*", 30)?.updatedAt, 30);
    assert.equal(deleteCustomRule(cache, "Work", "one"), true);
    assert.deepEqual(cache.groups[0].customRules, []);
});

test("renaming a group changes its persistent identity", () => {
    const cache = normalizeCache({ groups: [group("Old")] });
    const renamed = renameGroup(cache, "Old", "New", 10);
    assert.equal(renamed?.id, "New");
    assert.equal(renamed?.title, "New");
    assert.equal(cache.groups[0].id, "New");
});

test("pinning a page moves matching URL ownership to its latest group", () => {
    const cache = normalizeCache({
        groups: [
            group("work", [page("https://example.com/a?old=1", 1)]),
            group("later"),
        ],
    });
    assert.equal(
        recordPage(
            cache,
            "later",
            { title: "New", url: "https://example.com/a?new=2" },
            "full_path",
            10,
        ),
        true,
    );
    assert.equal(cache.groups[0].pages.length, 0);
    assert.equal(findGroupForUrl(cache, "https://example.com/a?x=3", "full_path")?.id, "later");
});

test("full-path pinning ignores a mapping already in the current group", () => {
    const cache = normalizeCache({
        groups: [
            group("Work"),
            group("Other", [page("https://example.com/a?other=1", 2)]),
        ],
    });
    assert.deepEqual(
        pinPageMapping(
            cache,
            "Work",
            { title: "Current", url: "https://example.com/a?new=1#part" },
            "full_path",
            10,
        ),
        { key: "https://example.com/a" },
    );
    assert.equal(cache.groups[0].pages.length, 1);
    assert.equal(cache.groups[1].pages.length, 0);

    assert.equal(
        pinPageMapping(
            cache,
            "Work",
            { title: "Ignored", url: "https://example.com/a?ignored=1" },
            "full_path",
            20,
        ),
        null,
    );
    assert.equal(cache.groups[0].pages.length, 1);
    assert.equal(cache.groups[0].pages[0].updatedAt, 10);
    assert.equal(cache.groups[1].pages.length, 0);
});

test("explicit ungrouping removes every mapping at the selected breadth", () => {
    const cache = normalizeCache({
        groups: [group("work", [
            page("https://example.com/a?x=1", 1),
            page("https://example.com/a?x=2", 2),
            page("https://example.com/b", 3),
        ])],
    });
    assert.equal(removeUrlMapping(cache, "https://example.com/a?x=9", "full_path"), true);
    assert.deepEqual(cache.groups[0].pages.map(({ url }) => url), ["https://example.com/b"]);
});

test("deleting a saved page removes the entire full-path matching item", () => {
    const cache = normalizeCache({
        groups: [group("work", [
            page("https://example.com/a?x=1", 1),
            page("https://example.com/a?x=2", 2),
            page("https://example.com/b", 3),
        ])],
    });
    assert.equal(
        deletePageMapping(cache, "work", "https://example.com/a?x=1", "full_path"),
        true,
    );
    assert.deepEqual(cache.groups[0].pages.map(({ url }) => url), ["https://example.com/b"]);
});

test("reindexing keeps the newest cross-group owner", () => {
    const cache = normalizeCache({
        groups: [
            group("older", [page("https://example.com/a?x=1", 10)]),
            group("newer", [page("https://example.com/a?x=2", 20)]),
        ],
    });
    reindexCache(cache, "full_path");
    assert.equal(cache.groups[0].pages.length, 0);
    assert.equal(cache.groups[1].pages.length, 1);
});
