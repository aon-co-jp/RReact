//! 仮想DOMの差分計算(reconciliation)。Reactの「同じ位置・同じ型なら
//! 更新、型が違えば置き換え、`key`付きの子要素はkeyで同一性を追跡する」
//! という基本方針を再現する。Fiberによる中断可能なレンダリング・
//! 優先度付きスケジューリングは次段階の課題として明記(第一段は
//! 「木を丸ごと比較してパッチ列を1回で作る」同期的な実装)。

use std::collections::HashMap;

use crate::vnode::{VElement, VNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Patch {
    /// この位置のノードを丸ごと新しいノードへ置き換える
    /// (タグ名が変わった、テキスト⇔要素が入れ替わった等)。
    Replace(VNode),
    /// テキストノードの内容変更。
    UpdateText(String),
    /// 同じ要素(タグ・keyが同一)のin-place更新。属性・子要素の
    /// どちらか一方、または両方が変化した場合にここへ集約される
    /// (両方Noneになることはない——変化が無ければ`Patch::NoOp`を返す)。
    UpdateElement { attrs: Option<AttrsPatch>, children: Option<Vec<ChildPatch>> },
    /// 変更なし。
    NoOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrsPatch {
    pub set: Vec<(String, String)>,
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildPatch {
    /// 新しい子ノードを`index`の位置に挿入する。
    Insert { index: usize, node: VNode },
    /// `index`位置の子ノードを削除する。
    Remove { index: usize },
    /// 既存の子ノード同士の差分(`old_index`にあった子を`new_index`の
    /// 位置に保ちつつ、その内部を`patch`で更新する)。
    Update { old_index: usize, new_index: usize, patch: Box<Patch> },
}

pub fn diff(old: &VNode, new: &VNode) -> Patch {
    match (old, new) {
        (VNode::Text(old_text), VNode::Text(new_text)) => {
            if old_text == new_text {
                Patch::NoOp
            } else {
                Patch::UpdateText(new_text.clone())
            }
        }
        (VNode::Element(old_el), VNode::Element(new_el)) if old_el.tag == new_el.tag && old_el.key == new_el.key => {
            diff_element(old_el, new_el)
        }
        _ => {
            if old == new {
                Patch::NoOp
            } else {
                Patch::Replace(new.clone())
            }
        }
    }
}

fn diff_element(old_el: &VElement, new_el: &VElement) -> Patch {
    let mut set = Vec::new();
    let mut remove = Vec::new();
    for (name, new_value) in &new_el.attrs {
        match old_el.attrs.get(name) {
            Some(old_value) if old_value == new_value => {}
            _ => set.push((name.clone(), new_value.clone())),
        }
    }
    for name in old_el.attrs.keys() {
        if !new_el.attrs.contains_key(name) {
            remove.push(name.clone());
        }
    }
    let attrs = if set.is_empty() && remove.is_empty() { None } else { Some(AttrsPatch { set, remove }) };

    let children = diff_children(&old_el.children, &new_el.children);

    if attrs.is_none() && children.is_none() {
        Patch::NoOp
    } else {
        Patch::UpdateElement { attrs, children }
    }
}

/// keyが付いている子はkeyで同一性を追跡し、無い子は位置(index)で
/// 対応させる、という2段構えの簡略化した子要素差分アルゴリズム
/// (Reactの実装が使う最長増加部分列(LIS)ベースの最小移動数計算までは
/// 再現しない——正しさは保つが、移動が最適(最小手数)であることは
/// 保証しない、という第一段の割り切り)。
fn diff_children(old_children: &[VNode], new_children: &[VNode]) -> Option<Vec<ChildPatch>> {
    let old_keyed: HashMap<&str, usize> =
        old_children.iter().enumerate().filter_map(|(i, n)| n.key().map(|k| (k, i))).collect();

    let mut patches = Vec::new();
    let mut used_old_indices = std::collections::HashSet::new();

    for (new_index, new_child) in new_children.iter().enumerate() {
        let matched_old_index = new_child.key().and_then(|k| old_keyed.get(k).copied()).or_else(|| {
            // keyが無い子は、対応する古い位置(同じindex)がまだ未使用なら
            // それと比較する(単純な位置ベースの対応)。
            if new_index < old_children.len() && old_children[new_index].key().is_none() {
                Some(new_index)
            } else {
                None
            }
        });

        match matched_old_index {
            Some(old_index) if !used_old_indices.contains(&old_index) => {
                used_old_indices.insert(old_index);
                let inner = diff(&old_children[old_index], new_child);
                if !matches!(inner, Patch::NoOp) || old_index != new_index {
                    patches.push(ChildPatch::Update { old_index, new_index, patch: Box::new(inner) });
                }
            }
            _ => {
                patches.push(ChildPatch::Insert { index: new_index, node: new_child.clone() });
            }
        }
    }

    for (old_index, _) in old_children.iter().enumerate() {
        if !used_old_indices.contains(&old_index) {
            patches.push(ChildPatch::Remove { index: old_index });
        }
    }

    if patches.is_empty() {
        None
    } else {
        Some(patches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vnode::VNode;

    #[test]
    fn identical_trees_produce_no_op() {
        let a = VNode::element("div").attr("class", "x").child(VNode::text("hi")).build();
        let b = a.clone();
        assert_eq!(diff(&a, &b), Patch::NoOp);
    }

    #[test]
    fn text_change_produces_update_text() {
        let a = VNode::text("old");
        let b = VNode::text("new");
        assert_eq!(diff(&a, &b), Patch::UpdateText("new".to_string()));
    }

    #[test]
    fn different_tag_produces_replace() {
        let a = VNode::element("div").build();
        let b = VNode::element("span").build();
        assert_eq!(diff(&a, &b), Patch::Replace(b));
    }

    #[test]
    fn attribute_change_produces_update_element_with_attrs_only() {
        let a = VNode::element("div").attr("class", "a").build();
        let b = VNode::element("div").attr("class", "b").build();
        assert_eq!(
            diff(&a, &b),
            Patch::UpdateElement {
                attrs: Some(AttrsPatch { set: vec![("class".to_string(), "b".to_string())], remove: vec![] }),
                children: None,
            }
        );
    }

    #[test]
    fn removed_attribute_is_reported() {
        let a = VNode::element("div").attr("disabled", "").build();
        let b = VNode::element("div").build();
        assert_eq!(
            diff(&a, &b),
            Patch::UpdateElement {
                attrs: Some(AttrsPatch { set: vec![], remove: vec!["disabled".to_string()] }),
                children: None,
            }
        );
    }

    #[test]
    fn appending_a_child_produces_an_insert_patch() {
        let a = VNode::element("ul").child(VNode::text("a")).build();
        let b = VNode::element("ul").child(VNode::text("a")).child(VNode::text("b")).build();
        assert_eq!(
            diff(&a, &b),
            Patch::UpdateElement {
                attrs: None,
                children: Some(vec![ChildPatch::Insert { index: 1, node: VNode::text("b") }]),
            }
        );
    }

    #[test]
    fn removing_a_child_produces_a_remove_patch() {
        let a = VNode::element("ul").child(VNode::text("a")).child(VNode::text("b")).build();
        let b = VNode::element("ul").child(VNode::text("a")).build();
        assert_eq!(
            diff(&a, &b),
            Patch::UpdateElement { attrs: None, children: Some(vec![ChildPatch::Remove { index: 1 }]) }
        );
    }

    #[test]
    fn attribute_and_children_changes_are_both_reported_together() {
        let a = VNode::element("div").attr("class", "a").child(VNode::text("x")).build();
        let b = VNode::element("div").attr("class", "b").child(VNode::text("x")).child(VNode::text("y")).build();
        assert_eq!(
            diff(&a, &b),
            Patch::UpdateElement {
                attrs: Some(AttrsPatch { set: vec![("class".to_string(), "b".to_string())], remove: vec![] }),
                children: Some(vec![ChildPatch::Insert { index: 1, node: VNode::text("y") }]),
            }
        );
    }

    #[test]
    fn keyed_children_are_tracked_by_identity_across_reordering() {
        let a = VNode::element("ul")
            .child(VNode::element("li").key("a").child(VNode::text("A")).build())
            .child(VNode::element("li").key("b").child(VNode::text("B")).build())
            .build();
        let b = VNode::element("ul")
            .child(VNode::element("li").key("b").child(VNode::text("B")).build())
            .child(VNode::element("li").key("a").child(VNode::text("A")).build())
            .build();

        let patch = diff(&a, &b);
        // 並び替えのみで内容(inner patch)自体は変わらないため、両方とも
        // "old_index != new_index"を理由にUpdateパッチとして現れる。
        let Patch::UpdateElement { children: Some(patches), .. } = patch else {
            panic!("expected children patch")
        };
        assert_eq!(patches.len(), 2);
        assert!(patches.iter().all(|p| matches!(p, ChildPatch::Update { .. })));
    }
}
