# RReact

React(React DOM・React Native・React Mobileのコンポーネントモデル)を、
一から開発するプロジェクト(`RHTML5`/`RCSS3`/`RTypeScript`/`RBootStrap`構想とは別の並行構想)。

## 現状

仮想DOM(`VNode`)とツリー差分計算(`diff`)のみ実装済み。
コンポーネントモデル・実DOMへのパッチ適用は未着手。

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
