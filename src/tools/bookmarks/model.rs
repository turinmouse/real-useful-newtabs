use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkNode {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub children: Vec<BookmarkNode>,
}

impl BookmarkNode {
    /// Recursively finds the node matching `target_id`.
    pub fn find_node<'a>(roots: &'a [BookmarkNode], target_id: &str) -> Option<&'a BookmarkNode> {
        for node in roots {
            if node.id == target_id {
                return Some(node);
            }
            if let Some(found) = Self::find_node(&node.children, target_id) {
                return Some(found);
            }
        }
        None
    }

    /// Recursively renames the node matching `target_id`.
    pub fn rename_node(roots: &mut [BookmarkNode], target_id: &str, new_title: &str) -> bool {
        for node in roots {
            if node.id == target_id {
                node.title = new_title.to_owned();
                return true;
            }
            if Self::rename_node(&mut node.children, target_id, new_title) {
                return true;
            }
        }
        false
    }

    /// Recursively removes the node matching `target_id` from the tree.
    pub fn delete_node(roots: &mut Vec<BookmarkNode>, target_id: &str) -> bool {
        if let Some(pos) = roots.iter().position(|node| node.id == target_id) {
            roots.remove(pos);
            return true;
        }
        for node in roots {
            if Self::delete_node(&mut node.children, target_id) {
                return true;
            }
        }
        false
    }

    /// Moves a node into a folder and returns Chrome's destination.
    pub fn move_into(
        roots: &mut Vec<BookmarkNode>,
        moving_id: &str,
        folder_id: &str,
    ) -> Option<(String, usize)> {
        if moving_id == folder_id || Self::is_descendant(roots, moving_id, folder_id) {
            return None;
        }

        let folder = Self::find_node(roots, folder_id)?;
        if folder.url.is_some() {
            return None;
        }
        let moving = Self::take_node(roots, moving_id)?;
        let folder = Self::find_node_mut(roots, folder_id)?;
        let destination_index = folder.children.len();
        folder.children.push(moving);
        Some((folder_id.to_owned(), destination_index))
    }

    /// Moves a node before a target item and returns the destination parent/index.
    pub fn move_before(
        roots: &mut Vec<BookmarkNode>,
        moving_id: &str,
        target_id: &str,
    ) -> Option<(String, usize)> {
        if moving_id == target_id || Self::is_descendant(roots, moving_id, target_id) {
            return None;
        }

        let (moving_parent, moving_index) = Self::find_parent_position(roots, moving_id)?;
        let (target_parent, target_index) = Self::find_parent_position(roots, target_id)?;
        if moving_parent == target_parent {
            let parent = Self::find_node_mut(roots, &moving_parent)?;
            let moving = parent.children.remove(moving_index);
            let insert_index =
                target_index.saturating_sub(usize::from(moving_index < target_index));
            parent.children.insert(insert_index, moving);
            return Some((moving_parent, insert_index));
        }

        let moving = Self::take_node(roots, moving_id)?;
        let target_parent_node = Self::find_node_mut(roots, &target_parent)?;
        let insert_index = target_index.min(target_parent_node.children.len());
        target_parent_node.children.insert(insert_index, moving);
        Some((target_parent, insert_index))
    }

    fn find_parent_position(roots: &[BookmarkNode], child_id: &str) -> Option<(String, usize)> {
        for node in roots {
            if let Some(index) = node.children.iter().position(|child| child.id == child_id) {
                return Some((node.id.clone(), index));
            }
            if let Some(found) = Self::find_parent_position(&node.children, child_id) {
                return Some(found);
            }
        }
        None
    }

    fn find_node_mut<'a>(
        roots: &'a mut [BookmarkNode],
        target_id: &str,
    ) -> Option<&'a mut BookmarkNode> {
        for node in roots {
            if node.id == target_id {
                return Some(node);
            }
            if let Some(found) = Self::find_node_mut(&mut node.children, target_id) {
                return Some(found);
            }
        }
        None
    }

    fn take_node(roots: &mut Vec<BookmarkNode>, target_id: &str) -> Option<BookmarkNode> {
        if let Some(index) = roots.iter().position(|node| node.id == target_id) {
            return Some(roots.remove(index));
        }
        for node in roots {
            if let Some(found) = Self::take_node(&mut node.children, target_id) {
                return Some(found);
            }
        }
        None
    }

    pub fn is_descendant(roots: &[BookmarkNode], ancestor_id: &str, descendant_id: &str) -> bool {
        Self::find_node(roots, ancestor_id)
            .is_some_and(|node| Self::find_node(&node.children, descendant_id).is_some())
    }

    /// Filters the bookmark tree by matching title or URL against the search query.
    pub fn filter_tree(roots: &[BookmarkNode], query: &str) -> Vec<BookmarkNode> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return roots.to_vec();
        }

        roots
            .iter()
            .filter_map(|node| node.filter_node(&q))
            .collect()
    }

    fn filter_node(&self, query: &str) -> Option<BookmarkNode> {
        let title_matches = self.title.to_lowercase().contains(query);
        let url_matches = self
            .url
            .as_ref()
            .is_some_and(|u| u.to_lowercase().contains(query));

        if title_matches || url_matches {
            Some(self.clone())
        } else {
            let filtered_children: Vec<BookmarkNode> = self
                .children
                .iter()
                .filter_map(|child| child.filter_node(query))
                .collect();

            if !filtered_children.is_empty() {
                Some(BookmarkNode {
                    id: self.id.clone(),
                    title: self.title.clone(),
                    url: self.url.clone(),
                    children: filtered_children,
                })
            } else {
                None
            }
        }
    }
}
