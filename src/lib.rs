//! RReact — React(React Native / React Mobileのコンポーネントモデルを
//! 含む)相当を、既存のReact/React DOM/React Nativeのコードを一切流用
//! せず一から開発するプロジェクト(`RHTML5/RCSS3/RTypeScript/
//! RBootStrap`とは別の並行構想、2026-07-18)。
//!
//! ## 現状(第三段、2026-09-13)
//! 仮想DOM(`vnode`)とツリー差分計算(`diff`)、`dom_bridge`フィーチャで
//! `Patch`の実DOM(`rhtml5::Node`)への適用(`dom_bridge::apply_patch`)に
//! 加え、コンポーネントモデル第一段(関数コンポーネント+`use_state`
//! フック、`hooks`モジュール)を実装済み。React Native/React Mobile
//! 相当の非HTMLターゲットへのレンダリングは引き続き未着手。

pub mod app;
pub mod diff;
pub mod hooks;
pub mod vnode;

#[cfg(feature = "dom_bridge")]
pub mod dom_bridge;

pub use app::App;
pub use diff::{AttrsPatch, ChildPatch, Patch};
pub use hooks::{run_component, take_dirty, use_state, ComponentId, StateSetter};
pub use vnode::{VElement, VNode};

#[cfg(feature = "dom_bridge")]
pub use dom_bridge::{apply_patch, render_to_html, render_to_vnode, ElementRef};
