#![warn(clippy::all, rust_2018_idioms)]

pub mod app;
pub mod canvas;
mod navigation;
pub mod state;
pub mod storage;
pub mod theme;
pub mod tools;

pub use app::NewTabsApp;
pub use canvas::{
    Canvas, CanvasLayout, CanvasWindowId, ShellLayout, SplitAxis, TileDropRegion, TileNode,
    TilingLayout, WindowLayout,
};
pub use state::{AppState, NewTabFocusPreference, ThemePreference};
