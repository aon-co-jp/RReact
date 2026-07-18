//! RReact — React(React Native / React Mobileのコンポーネントモデルを
//! 含む)相当を、既存のReact/React DOM/React Nativeのコードを一切流用
//! せず一から開発するプロジェクト(`RHTML5/RCSS3/RTypeScript/
//! RBootStrap`とは別の並行構想、2026-07-18)。
//!
//! ## 現状(第二段、2026-07-19)
//! 仮想DOM(`vnode`)とツリー差分計算(`diff`)に加え、`dom_bridge`
//! フィーチャで`Patch`の実DOM(`rhtml5::Node`)への適用
//! (`dom_bridge::apply_patch`)まで実装済み。コンポーネントモデル
//! (関数コンポーネント・hooks相当)・React Native/React Mobile相当の
//! 非HTMLターゲットへのレンダリングは引き続き未着手。

pub mod diff;
pub mod vnode;

#[cfg(feature = "dom_bridge")]
pub mod dom_bridge;

pub use diff::{AttrsPatch, ChildPatch, Patch};
pub use vnode::{VElement, VNode};

#[cfg(feature = "dom_bridge")]
pub use dom_bridge::{apply_patch, render_to_vnode, ElementRef};
