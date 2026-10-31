use super::*;
use crate::canvas::{BookmarkFolderWindowState, CanvasLayout};

#[test]
fn bookmark_drop_slots_only_accept_valid_siblings() {
    assert!(BookmarksTool::can_drop_before(
        false,
        Some("parent"),
        Some("moving"),
        "target"
    ));
    assert!(!BookmarksTool::can_drop_before(
        true,
        Some("parent"),
        Some("moving"),
        "target"
    ));
    assert!(!BookmarksTool::can_drop_before(
        false,
        None,
        Some("moving"),
        "target"
    ));
    assert!(!BookmarksTool::can_drop_before(
        false,
        Some("parent"),
        Some("target"),
        "target"
    ));
}

#[test]
fn bookmark_drop_slot_indicator_does_not_change_layout() {
    let ctx = egui::Context::default();
    let mut slot_position = egui::Pos2::ZERO;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        slot_position = egui::pos2(ui.cursor().center().x, ui.cursor().top());
    })
    .drop_without_applying_deltas();

    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(slot_position)],
            ..Default::default()
        },
        |ui| {
            egui::DragAndDrop::set_payload(ui.ctx(), "moving".to_owned());
            BookmarksTool::drop_slot(ui, Some("parent"), "target", false, &mut None);
        },
    )
    .drop_without_applying_deltas();

    let mut cursor_before = 0.0;
    let mut cursor_after = 0.0;
    let output = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(slot_position)],
            ..Default::default()
        },
        |ui| {
            egui::DragAndDrop::set_payload(ui.ctx(), "moving".to_owned());
            cursor_before = ui.cursor().top();

            let mut pending_action = None;
            assert!(BookmarksTool::drop_slot(
                ui,
                Some("parent"),
                "target",
                false,
                &mut pending_action,
            ));

            cursor_after = ui.cursor().top();
        },
    );

    assert_eq!(cursor_after, cursor_before);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| matches!(shape.shape, egui::Shape::LineSegment { .. }))
    );
}

#[test]
fn empty_bookmark_title_has_a_fallback() {
    let node = BookmarkNode {
        id: "1".to_owned(),
        title: String::new(),
        url: None,
        children: Vec::new(),
    };

    assert_eq!(BookmarksTool::label(&node), "Untitled");
}

#[test]
fn non_empty_bookmark_title_is_preserved() {
    let node = BookmarkNode {
        id: "2".to_owned(),
        title: "GitHub".to_owned(),
        url: Some("https://github.com".to_owned()),
        children: Vec::new(),
    };

    assert_eq!(BookmarksTool::label(&node), "GitHub");
}

#[test]
fn rename_node_updates_matching_id() {
    let mut roots = vec![BookmarkNode {
        id: "1".to_owned(),
        title: "Folder".to_owned(),
        url: None,
        children: vec![BookmarkNode {
            id: "2".to_owned(),
            title: "Old Link".to_owned(),
            url: Some("https://example.com".to_owned()),
            children: Vec::new(),
        }],
    }];

    // Rename child link
    assert!(BookmarkNode::rename_node(&mut roots, "2", "New Link"));
    assert_eq!(roots[0].children[0].title, "New Link");

    // Rename parent folder
    assert!(BookmarkNode::rename_node(&mut roots, "1", "Renamed Folder"));
    assert_eq!(roots[0].title, "Renamed Folder");

    // Non-existent id returns false
    assert!(!BookmarkNode::rename_node(&mut roots, "999", "None"));
}

#[test]
fn move_node_reorders_and_moves_between_folders() {
    let mut roots = vec![BookmarkNode {
        id: "0".to_owned(),
        title: String::new(),
        url: None,
        children: vec![
            BookmarkNode {
                id: "1".to_owned(),
                title: "One".to_owned(),
                url: Some("https://one.example".to_owned()),
                children: Vec::new(),
            },
            BookmarkNode {
                id: "2".to_owned(),
                title: "Two".to_owned(),
                url: Some("https://two.example".to_owned()),
                children: Vec::new(),
            },
            BookmarkNode {
                id: "folder".to_owned(),
                title: "Folder".to_owned(),
                url: None,
                children: Vec::new(),
            },
            BookmarkNode {
                id: "other-folder".to_owned(),
                title: "Other Folder".to_owned(),
                url: None,
                children: vec![BookmarkNode {
                    id: "3".to_owned(),
                    title: "Three".to_owned(),
                    url: Some("https://three.example".to_owned()),
                    children: Vec::new(),
                }],
            },
        ],
    }];

    assert_eq!(
        BookmarkNode::move_before(&mut roots, "2", "1"),
        Some(("0".to_owned(), 0))
    );
    assert_eq!(
        roots[0]
            .children
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["2", "1", "folder", "other-folder"]
    );

    assert_eq!(
        BookmarkNode::move_into(&mut roots, "1", "folder"),
        Some(("folder".to_owned(), 0))
    );
    assert_eq!(roots[0].children[1].id, "folder");
    assert_eq!(roots[0].children[1].children[0].id, "1");
    assert!(BookmarkNode::move_into(&mut roots, "folder", "1").is_none());

    assert_eq!(
        BookmarkNode::move_before(&mut roots, "3", "2"),
        Some(("0".to_owned(), 0))
    );
    assert_eq!(roots[0].children[0].id, "3");
    assert_eq!(roots[0].children[1].id, "2");
    assert_eq!(roots[0].children[2].id, "folder");
    assert!(roots[0].children[3].children.is_empty());
}

#[test]
fn find_node_returns_nested_folder() {
    let roots = vec![BookmarkNode {
        id: "root".to_owned(),
        title: "Root".to_owned(),
        url: None,
        children: vec![BookmarkNode {
            id: "folder".to_owned(),
            title: "Folder".to_owned(),
            url: None,
            children: Vec::new(),
        }],
    }];

    assert_eq!(
        BookmarkNode::find_node(&roots, "folder").map(|node| node.title.as_str()),
        Some("Folder")
    );
    assert!(BookmarkNode::find_node(&roots, "missing").is_none());
}

#[test]
fn folder_window_reuses_same_canvas_and_folder() {
    let mut windows = Vec::new();
    BookmarkFolderWindowState::open_or_reuse(&mut windows, "folder".to_owned());
    BookmarkFolderWindowState::open_or_reuse(&mut windows, "folder".to_owned());
    BookmarkFolderWindowState::open_or_reuse(&mut windows, "other".to_owned());

    assert_eq!(windows.len(), 2);
    assert!(windows.iter().all(|window| window.window.open));
}

#[test]
fn bookmark_tree_defaults_expand_only_visible_root_folders() {
    let roots = vec![BookmarkNode {
        id: "hidden-root".to_owned(),
        title: String::new(),
        url: None,
        children: vec![
            BookmarkNode {
                id: "toolbar".to_owned(),
                title: "Toolbar".to_owned(),
                url: None,
                children: vec![BookmarkNode {
                    id: "nested".to_owned(),
                    title: "Nested".to_owned(),
                    url: None,
                    children: Vec::new(),
                }],
            },
            BookmarkNode {
                id: "root-link".to_owned(),
                title: "Link".to_owned(),
                url: Some("https://example.com".to_owned()),
                children: Vec::new(),
            },
        ],
    }];

    assert_eq!(
        BookmarksTool::default_expanded_folder_ids(&roots),
        vec!["toolbar"]
    );
}

#[test]
fn bookmark_windows_keep_legacy_expansion_defaults_isolated() {
    let mut layout = CanvasLayout::default();
    layout
        .bookmarks_tree
        .initialize(["toolbar".to_owned(), "nested".to_owned()]);
    BookmarkFolderWindowState::open_or_reuse(
        &mut layout.bookmark_folder_windows,
        "toolbar".to_owned(),
    );
    layout.bookmark_folder_windows[0]
        .tree
        .initialize(["folder-window-root".to_owned()]);

    assert!(layout.bookmarks_tree.is_expanded("toolbar"));
    assert!(layout.bookmarks_tree.is_expanded("nested"));
    assert!(
        layout.bookmark_folder_windows[0]
            .tree
            .is_expanded("folder-window-root")
    );
    assert!(!layout.bookmark_folder_windows[0].tree.is_expanded("nested"));
}

#[test]
fn delete_node_removes_matching_id() {
    let mut roots = vec![
        BookmarkNode {
            id: "1".to_owned(),
            title: "Folder".to_owned(),
            url: None,
            children: vec![
                BookmarkNode {
                    id: "2".to_owned(),
                    title: "Link 1".to_owned(),
                    url: Some("https://example.com/1".to_owned()),
                    children: Vec::new(),
                },
                BookmarkNode {
                    id: "3".to_owned(),
                    title: "Link 2".to_owned(),
                    url: Some("https://example.com/2".to_owned()),
                    children: Vec::new(),
                },
            ],
        },
        BookmarkNode {
            id: "4".to_owned(),
            title: "Root Link".to_owned(),
            url: Some("https://example.com/root".to_owned()),
            children: Vec::new(),
        },
    ];

    // Delete nested child
    assert!(BookmarkNode::delete_node(&mut roots, "2"));
    assert_eq!(roots[0].children.len(), 1);
    assert_eq!(roots[0].children[0].id, "3");

    // Delete root item
    assert!(BookmarkNode::delete_node(&mut roots, "4"));
    assert_eq!(roots.len(), 1);

    // Delete parent folder
    assert!(BookmarkNode::delete_node(&mut roots, "1"));
    assert!(roots.is_empty());

    // Non-existent id returns false
    assert!(!BookmarkNode::delete_node(&mut roots, "999"));
}

#[test]
fn filter_tree_matches_search_queries() {
    let roots = vec![
        BookmarkNode {
            id: "1".to_owned(),
            title: "Development".to_owned(),
            url: None,
            children: vec![
                BookmarkNode {
                    id: "2".to_owned(),
                    title: "GitHub".to_owned(),
                    url: Some("https://github.com".to_owned()),
                    children: Vec::new(),
                },
                BookmarkNode {
                    id: "3".to_owned(),
                    title: "Rust Lang".to_owned(),
                    url: Some("https://www.rust-lang.org".to_owned()),
                    children: Vec::new(),
                },
            ],
        },
        BookmarkNode {
            id: "4".to_owned(),
            title: "News".to_owned(),
            url: Some("https://news.ycombinator.com".to_owned()),
            children: Vec::new(),
        },
    ];

    // Empty query returns all
    let all = BookmarkNode::filter_tree(&roots, "");
    assert_eq!(all.len(), 2);

    // Search by title
    let rust_results = BookmarkNode::filter_tree(&roots, "rust");
    assert_eq!(rust_results.len(), 1);
    assert_eq!(rust_results[0].children.len(), 1);
    assert_eq!(rust_results[0].children[0].title, "Rust Lang");

    // Search by URL
    let ycomb_results = BookmarkNode::filter_tree(&roots, "ycombinator");
    assert_eq!(ycomb_results.len(), 1);
    assert_eq!(ycomb_results[0].id, "4");

    // Search by folder name
    let dev_results = BookmarkNode::filter_tree(&roots, "development");
    assert_eq!(dev_results.len(), 1);
    assert_eq!(dev_results[0].children.len(), 2);

    // No match returns empty
    let none_results = BookmarkNode::filter_tree(&roots, "nonexistent");
    assert!(none_results.is_empty());
}
