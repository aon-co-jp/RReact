# RReact

React(React DOM・React Native・React Mobileのコンポーネントモデル)を、
一から開発するプロジェクト(`RHTML5`/`RCSS3`/`RTypeScript`/`RBootStrap`構想とは別の並行構想)。

## 現状

仮想DOM(`VNode`)とツリー差分計算(`diff`)に加え、`dom_bridge`フィーチャ
(既定では無効)で`Patch`の実DOM(`rhtml5::Node`)への適用
(`dom_bridge::apply_patch`)まで実装済み。ここでの「実DOM」は
`web-sys`のブラウザDOMではなく、本クレートが接続する`rhtml5`(RHTML)の
`Node`/`Element`木のこと(この生態系に`web-sys`/`wasm-bindgen`は
存在しない)。

コンポーネントモデル(関数コンポーネント・hooks相当)は未着手。

## 使用例

```rust
use rreact::{diff, VNode};

let old = VNode::element("ul")
    .child(VNode::element("li").key("a").child(VNode::text("A")).build())
    .build();
let new = VNode::element("ul")
    .child(VNode::element("li").key("a").child(VNode::text("A!")).build())
    .build();

let patch = diff(&old, &new);
println!("{:?}", patch);
```

## ビルド・テスト

```bash
cargo test
```

## ライセンス

Apache-2.0 OR MIT
