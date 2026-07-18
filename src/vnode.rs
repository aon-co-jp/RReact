//! 仮想DOM(Virtual DOM)の中核データ構造。ReactのVNode/Fiber木に相当
//! するが、まずは最小限の「要素木のスナップショット」表現から始める
//! (Fiberの中断可能なレンダリング等は次段階の課題として明記)。

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VNode {
    Element(VElement),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VElement {
    pub tag: String,
    /// 属性は決定的な差分計算のため`BTreeMap`(順序が安定する)。
    pub attrs: BTreeMap<String, String>,
    /// Reactの`key`プロパティに相当。リストの子要素の同一性追跡に使う。
    pub key: Option<String>,
    pub children: Vec<VNode>,
}

impl VNode {
    pub fn element(tag: impl Into<String>) -> VElementBuilder {
        VElementBuilder {
            tag: tag.into(),
            attrs: BTreeMap::new(),
            key: None,
            children: Vec::new(),
        }
    }

    pub fn text(value: impl Into<String>) -> VNode {
        VNode::Text(value.into())
    }

    pub fn key(&self) -> Option<&str> {
        match self {
            VNode::Element(el) => el.key.as_deref(),
            VNode::Text(_) => None,
        }
    }
}

/// `VElement`を組み立てるための小さなビルダー(呼び出し側が
/// `VNode::element("div").attr("class", "a").child(...)`のように
/// 書けるようにする、Reactの`React.createElement`相当)。
pub struct VElementBuilder {
    tag: String,
    attrs: BTreeMap<String, String>,
    key: Option<String>,
    children: Vec<VNode>,
}

impl VElementBuilder {
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(name.into(), value.into());
        self
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn child(mut self, child: VNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = VNode>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn build(self) -> VNode {
        VNode::Element(VElement { tag: self.tag, attrs: self.attrs, key: self.key, children: self.children })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_produces_expected_element_shape() {
        let node = VNode::element("div")
            .attr("class", "greeting")
            .key("row-1")
            .child(VNode::text("Hello"))
            .build();

        let VNode::Element(el) = node else { panic!("expected element") };
        assert_eq!(el.tag, "div");
        assert_eq!(el.attrs.get("class"), Some(&"greeting".to_string()));
        assert_eq!(el.key.as_deref(), Some("row-1"));
        assert_eq!(el.children, vec![VNode::text("Hello")]);
    }
}
