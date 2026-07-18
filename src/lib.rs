//! RReact — React(React Native / React Mobileのコンポーネントモデルを
//! 含む)相当を、既存のReact/React DOM/React Nativeのコードを一切流用
//! せず一から開発するプロジェクト(`RHTML5/RCSS3/RTypeScript/
//! RBootStrap`とは別の並行構想、2026-07-18)。
//!
//! ## 現状(第一段)
//! 仮想DOM(`vnode`)とツリー差分計算(`diff`)のみ。コンポーネント
//! モデル(関数コンポーネント・hooks相当)・実DOMへのパッチ適用
//! (`rhtml5`との連携)・React Native/React Mobile相当の非HTML
//! ターゲットへのレンダリングは未着手。

pub mod diff;
pub mod vnode;

#[cfg(feature = "dom_bridge")]
pub mod dom_bridge;

pub use diff::{AttrsPatch, ChildPatch, Patch};
pub use vnode::{VElement, VNode};

#[cfg(feature = "dom_bridge")]
pub use dom_bridge::{render_to_vnode, ElementRef};
