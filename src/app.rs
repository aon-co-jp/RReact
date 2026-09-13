//! ツリー全体の再レンダーループ(「アプリループ」)。
//!
//! `hooks`モジュールが個々のコンポーネントインスタンスのフック状態を
//! 管理するのに対し、本モジュールは「dirtyなコンポーネントが1つでも
//! あれば、ルートから丸ごと再レンダーしてdiffを取る」という
//! アプリ全体のループを提供する。
//!
//! **設計上の割り切り(v1)**: dirtyになったコンポーネント個別だけを
//! 狙って再レンダーする最適化(Reactの実装が行う「差分検知して
//! そのサブツリーだけ再render」)はしない。ルートの`render_fn`を
//! 丸ごと呼び直すことで、ネストした子コンポーネント(`render_fn`の中で
//! さらに`hooks::run_component`を呼ぶ形で表現する——本クレートに
//! `VNode::Component`のような専用バリアントは無く、コンポーネント木は
//! 「render関数がネストして他のrender関数を呼ぶ」という形でのみ存在
//! する)も含めて最新のフック状態で再評価される。正しさを優先し、
//! 性能上の最適化は次段階の課題とする(子要素差分の最小移動数計算が
//! 「正しいが最適とは限らない」とされているのと同じ割り切り)。

use crate::diff::diff;
use crate::hooks::{any_dirty, clear_all_dirty, run_component, ComponentId};
use crate::vnode::VNode;
use crate::Patch;

#[cfg(feature = "dom_bridge")]
use crate::dom_bridge::apply_patch;
#[cfg(feature = "dom_bridge")]
use rhtml5::Node;

/// マウント済みのアプリケーション。`root_id`はルートコンポーネントの
/// `ComponentId`(ツリー上の位置や`key`から呼び出し側が決定する
/// `hooks`モジュールの規約と同じ)、`render_fn`はルートのrender関数。
pub struct App<F: Fn() -> VNode> {
    root_id: ComponentId,
    render_fn: F,
    tree: VNode,
}

impl<F: Fn() -> VNode> App<F> {
    /// `render_fn`を初回実行し、その結果を現在のツリーとして保持する
    /// (Reactの初回`render`相当、この時点ではdiff/patchは発生しない)。
    pub fn mount(root_id: ComponentId, render_fn: F) -> Self {
        let tree = run_component(root_id, || render_fn());
        Self { root_id, render_fn, tree }
    }

    /// 現在保持しているツリー(直近の`mount`/`tick`の結果)。
    pub fn tree(&self) -> &VNode {
        &self.tree
    }

    /// このアプリのどこかのコンポーネントがdirty(状態更新済み)なら、
    /// ルートから再レンダーしてdiffを取り、`Patch`を返す。dirtyな
    /// コンポーネントが無ければ`None`(再レンダー自体を行わない
    /// ——`take_dirty`を毎回消費してしまう素朴なポーリングループとの
    /// 違いはここ、無駄な再レンダーを避ける)。
    pub fn tick(&mut self) -> Option<Patch> {
        if !any_dirty() {
            return None;
        }
        // 丸ごと再レンダーする以上、個々のインスタンスのdirtyフラグを
        // 選別する必要は無い——まとめて消費する。
        clear_all_dirty();
        let new_tree = run_component(self.root_id, || (self.render_fn)());
        let patch = diff(&self.tree, &new_tree);
        self.tree = new_tree;
        Some(patch)
    }

    /// `tick`に加えて、dirtyだった場合は`Patch`を実DOM(`rhtml5::Node`)
    /// へ即座に適用する。適用が実際に行われたかを返す。
    #[cfg(feature = "dom_bridge")]
    pub fn tick_and_apply(&mut self, real: &mut Node) -> bool {
        match self.tick() {
            Some(patch) => {
                apply_patch(real, &patch);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{drop_instance, use_state};
    use crate::vnode::VNode;

    #[test]
    fn tick_returns_none_when_nothing_is_dirty() {
        let mut app = App::mount(100, || VNode::text("hello"));
        assert!(app.tick().is_none());
        drop_instance(100);
    }

    #[test]
    fn tick_re_renders_and_diffs_when_root_state_changes() {
        let root_id = 101;
        let setter = run_component(root_id, || {
            let (_count, set) = use_state(|| 0i32);
            set
        });
        let mut app = App::mount(root_id, move || {
            let (count, _set) = use_state(|| 0i32);
            VNode::text(&count.to_string())
        });
        assert!(matches!(app.tree(), VNode::Text(t) if t == "0"));

        setter.set(1);
        let patch = app.tick().expect("state change must produce a patch");
        assert!(matches!(patch, Patch::UpdateText(ref t) if t == "1"));
        assert!(matches!(app.tree(), VNode::Text(t) if t == "1"));

        assert!(app.tick().is_none(), "tick must be idempotent once nothing is dirty");
        drop_instance(root_id);
    }

    #[test]
    fn tick_re_evaluates_nested_components_defined_inside_the_render_fn() {
        let root_id = 102;
        let child_id = 103;
        let child_setter = run_component(child_id, || {
            let (_count, set) = use_state(|| 0i32);
            set
        });
        let mut app = App::mount(root_id, move || {
            run_component(child_id, || {
                let (count, _set) = use_state(|| 0i32);
                VNode::text(&format!("child:{count}"))
            })
        });
        assert!(matches!(app.tree(), VNode::Text(t) if t == "child:0"));

        child_setter.set(5);
        let patch = app.tick().expect("nested component's state change must be observed by the app loop");
        assert!(matches!(patch, Patch::UpdateText(ref t) if t == "child:5"));

        drop_instance(root_id);
        drop_instance(child_id);
    }
}
