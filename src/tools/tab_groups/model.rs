use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedTabGroup {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub pages: Vec<SavedTabPage>,
    #[serde(default)]
    pub custom_rules: Vec<SavedTabRule>,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedTabPage {
    #[serde(default)]
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedTabRule {
    pub id: String,
    pub pattern: String,
    #[serde(default)]
    pub updated_at: u64,
}

fn default_color() -> String {
    "grey".to_owned()
}

impl SavedTabGroup {
    pub fn label(&self) -> &str {
        if self.title.trim().is_empty() {
            "Unnamed group"
        } else {
            &self.title
        }
    }

    pub fn sort_contents(groups: &mut [Self]) {
        for group in groups.iter_mut() {
            group.custom_rules.sort_by(|left, right| {
                right
                    .updated_at
                    .cmp(&left.updated_at)
                    .then_with(|| left.id.cmp(&right.id))
            });
            group.pages.sort_by(|left, right| {
                left.label()
                    .to_lowercase()
                    .cmp(&right.label().to_lowercase())
                    .then_with(|| left.url.cmp(&right.url))
            });
        }
        groups.sort_by(|left, right| {
            left.label()
                .to_lowercase()
                .cmp(&right.label().to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
    }

    pub fn filter_by_address(groups: &[Self], query: &str) -> Vec<Self> {
        let query = query.trim();
        if query.is_empty() {
            return groups.to_vec();
        }
        let lowercase_query = query.to_lowercase();

        groups
            .iter()
            .filter_map(|group| {
                let mut filtered = group.clone();
                filtered
                    .custom_rules
                    .retain(|rule| rule.matches_address(query, &lowercase_query));
                filtered
                    .pages
                    .retain(|page| page.url.to_lowercase().contains(&lowercase_query));
                (!filtered.custom_rules.is_empty() || !filtered.pages.is_empty())
                    .then_some(filtered)
            })
            .collect()
    }
}

impl SavedTabPage {
    pub fn label(&self) -> &str {
        if self.title.trim().is_empty() {
            &self.url
        } else {
            &self.title
        }
    }
}

impl SavedTabRule {
    fn matches_address(&self, query: &str, lowercase_query: &str) -> bool {
        self.pattern.to_lowercase().contains(lowercase_query) || glob_matches(&self.pattern, query)
    }
}

#[derive(Clone, Copy)]
enum GlobToken {
    AnySequence,
    AnyCharacter,
    Literal(char),
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let mut tokens = Vec::new();
    let mut characters = pattern.chars();
    while let Some(character) = characters.next() {
        match character {
            '*' => tokens.push(GlobToken::AnySequence),
            '?' => tokens.push(GlobToken::AnyCharacter),
            '\\' => {
                let Some(literal) = characters.next() else {
                    return false;
                };
                tokens.push(GlobToken::Literal(literal));
            }
            literal => tokens.push(GlobToken::Literal(literal)),
        }
    }

    let value: Vec<char> = value.chars().collect();
    let mut matches = vec![false; value.len() + 1];
    matches[0] = true;
    for token in tokens {
        let mut next = vec![false; value.len() + 1];
        match token {
            GlobToken::AnySequence => {
                next[0] = matches[0];
                for index in 1..=value.len() {
                    next[index] = matches[index] || next[index - 1];
                }
            }
            GlobToken::AnyCharacter => {
                next[1..].copy_from_slice(&matches[..value.len()]);
            }
            GlobToken::Literal(literal) => {
                for index in 1..=value.len() {
                    next[index] = matches[index - 1] && value[index - 1] == literal;
                }
            }
        }
        matches = next;
    }
    matches[value.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_titles_use_readable_fallbacks() {
        let group = SavedTabGroup {
            id: "group".to_owned(),
            title: "  ".to_owned(),
            color: "grey".to_owned(),
            pages: Vec::new(),
            custom_rules: Vec::new(),
            updated_at: 0,
        };
        let page = SavedTabPage {
            title: String::new(),
            url: "https://example.com".to_owned(),
            updated_at: 0,
        };
        assert_eq!(group.label(), "Unnamed group");
        assert_eq!(page.label(), "https://example.com");
    }

    #[test]
    fn groups_and_pages_sort_by_visible_label() {
        let mut groups = vec![
            SavedTabGroup {
                id: "z".to_owned(),
                title: "Zulu".to_owned(),
                color: "grey".to_owned(),
                pages: vec![
                    SavedTabPage {
                        title: "Beta".to_owned(),
                        url: "https://example.com/b".to_owned(),
                        updated_at: 0,
                    },
                    SavedTabPage {
                        title: "alpha".to_owned(),
                        url: "https://example.com/a".to_owned(),
                        updated_at: 0,
                    },
                ],
                custom_rules: vec![
                    SavedTabRule {
                        id: "older".to_owned(),
                        pattern: "https://old.example/*".to_owned(),
                        updated_at: 1,
                    },
                    SavedTabRule {
                        id: "newer".to_owned(),
                        pattern: "https://new.example/*".to_owned(),
                        updated_at: 2,
                    },
                ],
                updated_at: 0,
            },
            SavedTabGroup {
                id: "a".to_owned(),
                title: "Alpha".to_owned(),
                color: "blue".to_owned(),
                pages: Vec::new(),
                custom_rules: Vec::new(),
                updated_at: 0,
            },
        ];
        SavedTabGroup::sort_contents(&mut groups);
        assert_eq!(groups[0].title, "Alpha");
        assert_eq!(groups[1].pages[0].title, "alpha");
        assert_eq!(groups[1].custom_rules[0].id, "newer");
    }

    #[test]
    fn older_group_data_defaults_to_no_custom_rules() {
        let group: SavedTabGroup =
            ron::from_str("(id:\"Work\",title:\"Work\",color:\"blue\",pages:[],updatedAt:1)")
                .unwrap();
        assert!(group.custom_rules.is_empty());
    }

    #[test]
    fn glob_matching_supports_wildcards_and_escaping() {
        assert!(glob_matches(
            "https://*.example.com/docs/*",
            "https://sub.example.com/docs/page"
        ));
        assert!(glob_matches(
            r"https://example.com/search\?q=te?t",
            "https://example.com/search?q=test"
        ));
        assert!(!glob_matches(
            "https://example.com/docs",
            "https://example.com/docs/page"
        ));
        assert!(!glob_matches(
            "https://example.com/*\\",
            "https://example.com/a"
        ));
    }

    #[test]
    fn address_filter_keeps_only_matching_records_and_groups() {
        let groups = vec![
            SavedTabGroup {
                id: "work".to_owned(),
                title: "Work".to_owned(),
                color: "blue".to_owned(),
                pages: vec![
                    SavedTabPage {
                        title: "Ignored title match".to_owned(),
                        url: "https://example.com/docs/page".to_owned(),
                        updated_at: 1,
                    },
                    SavedTabPage {
                        title: "example.com/docs".to_owned(),
                        url: "https://other.example/page".to_owned(),
                        updated_at: 2,
                    },
                ],
                custom_rules: vec![SavedTabRule {
                    id: "rule".to_owned(),
                    pattern: "*://example.com/docs/*".to_owned(),
                    updated_at: 3,
                }],
                updated_at: 3,
            },
            SavedTabGroup {
                id: "hidden".to_owned(),
                title: "example.com/docs".to_owned(),
                color: "grey".to_owned(),
                pages: Vec::new(),
                custom_rules: Vec::new(),
                updated_at: 0,
            },
        ];

        let filtered = SavedTabGroup::filter_by_address(&groups, "EXAMPLE.COM/DOCS");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].custom_rules.len(), 1);
        assert_eq!(filtered[0].pages.len(), 1);
        assert_eq!(filtered[0].pages[0].url, "https://example.com/docs/page");
    }

    #[test]
    fn address_filter_uses_glob_matching_for_complete_urls() {
        let groups = vec![SavedTabGroup {
            id: "work".to_owned(),
            title: "Work".to_owned(),
            color: "blue".to_owned(),
            pages: Vec::new(),
            custom_rules: vec![SavedTabRule {
                id: "rule".to_owned(),
                pattern: "*://example.com/*".to_owned(),
                updated_at: 1,
            }],
            updated_at: 1,
        }];

        let filtered = SavedTabGroup::filter_by_address(&groups, "https://example.com/private");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].custom_rules[0].id, "rule");
    }
}
