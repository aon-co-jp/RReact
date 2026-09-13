//! コンポーネントモデル第一段: 関数コンポーネント + `use_state`フック。
//!
//! Reactの「フックはコンポーネントごとに呼び出し順序が一定」という
//! ルールと同じ制約を踏襲する——`use_state`はコンポーネントの
//! render関数の中で、分岐やループの外側で呼ぶ前提(呼び出し順序が
//! そのままフックの識別子になる、Reactの実装と同じ割り切り)。
//!
//! 状態はコンポーネントインスタンスの識別子(`ComponentId`、呼び出し側が
//! 割り当てる——ツリー上の位置や明示的な`key`から決める想定)に紐づけて
//! `HOOK_STORE`(スレッドローカル)に永続化し、再レンダー(`render`の
//! 再実行)をまたいで保持する。

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

/// コンポーネントインスタンスの識別子。呼び出し側(アプリのレンダー
/// ループ)がツリー上の位置や`key`から決定し、同じインスタンスに対して
/// 再レンダーのたびに同じ値を渡す責務を持つ。
pub type ComponentId = u64;

struct HookFrame {
    states: Vec<Box<dyn Any>>,
    cursor: usize,
}

impl HookFrame {
    fn new() -> Self {
        Self { states: Vec::new(), cursor: 0 }
    }
}

thread_local! {
    static HOOK_STORE: RefCell<HashMap<ComponentId, HookFrame>> = RefCell::new(HashMap::new());
    static ACTIVE_STACK: RefCell<Vec<ComponentId>> = RefCell::new(Vec::new());
    static DIRTY: RefCell<std::collections::HashSet<ComponentId>> = RefCell::new(std::collections::HashSet::new());
}

/// コンポーネントのrender関数を呼び出す前に、そのインスタンスの
/// フックカーソルをリセットする。render関数呼び出しの直前に必ず対で
/// 呼ぶこと(`run_component`を使えば自動で対になる)。
fn begin_render(id: ComponentId) {
    HOOK_STORE.with(|store| {
        store.borrow_mut().entry(id).or_insert_with(HookFrame::new).cursor = 0;
    });
    ACTIVE_STACK.with(|stack| stack.borrow_mut().push(id));
}

fn end_render(_id: ComponentId) {
    ACTIVE_STACK.with(|stack| {
        stack.borrow_mut().pop();
    });
}

fn current_id() -> ComponentId {
    ACTIVE_STACK.with(|stack| {
        *stack
            .borrow()
            .last()
            .expect("use_state must be called during a component's render (no active component on the stack)")
    })
}

/// `render_fn`を、フックの状態を`id`に紐づけた状態で1回実行する。
/// `render_fn`の中で呼ばれる`use_state`は、この`id`のフックストアを見る。
pub fn run_component<T>(id: ComponentId, render_fn: impl FnOnce() -> T) -> T {
    begin_render(id);
    let result = render_fn();
    end_render(id);
    result
}

/// 直前の`run_component`呼び出し以降、このコンポーネントインスタンスが
/// 再レンダーを要求したか(`StateSetter`が呼ばれたか)を確認し、
/// フラグを消費する(一度読んだら消える——次の変化まで再度falseになる)。
pub fn take_dirty(id: ComponentId) -> bool {
    DIRTY.with(|dirty| dirty.borrow_mut().remove(&id))
}

/// 状態を更新するためのハンドル。`set`を呼ぶと即座にストアへ反映され、
/// 対応する`ComponentId`が`take_dirty`で観測可能になる
/// (Reactのように非同期にバッチングはしない、素朴な同期反映)。
pub struct StateSetter<T> {
    id: ComponentId,
    index: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> StateSetter<T> {
    pub fn set(&self, value: T) {
        HOOK_STORE.with(|store| {
            let mut store = store.borrow_mut();
            let frame = store.get_mut(&self.id).expect("hook frame must exist before its setter is used");
            frame.states[self.index] = Box::new(value);
        });
        DIRTY.with(|dirty| {
            dirty.borrow_mut().insert(self.id);
        });
    }

    /// 現在値を受け取って新しい値を返す関数で更新する(Reactの
    /// `setState(prev => next)`相当)。
    pub fn update(&self, f: impl FnOnce(&T) -> T) {
        let next = HOOK_STORE.with(|store| {
            let store = store.borrow();
            let frame = store.get(&self.id).expect("hook frame must exist before its setter is used");
            let current = frame.states[self.index].downcast_ref::<T>().expect("use_state type mismatch");
            f(current)
        });
        self.set(next);
    }
}

impl<T> Clone for StateSetter<T> {
    fn clone(&self) -> Self {
        Self { id: self.id, index: self.index, _marker: std::marker::PhantomData }
    }
}

/// `use_state`フック。現在の値のクローンと、更新用の`StateSetter`を返す。
/// `init`は初回(このインスタンス・このフック位置で初めて呼ばれた時)
/// のみ評価される。
pub fn use_state<T: Clone + 'static>(init: impl FnOnce() -> T) -> (T, StateSetter<T>) {
    let id = current_id();
    let index = HOOK_STORE.with(|store| {
        let mut store = store.borrow_mut();
        let frame = store.get_mut(&id).expect("use_state called outside of run_component");
        let index = frame.cursor;
        frame.cursor += 1;
        if index == frame.states.len() {
            frame.states.push(Box::new(init()));
        }
        index
    });
    let value = HOOK_STORE.with(|store| {
        let store = store.borrow();
        let frame = store.get(&id).unwrap();
        frame.states[index].downcast_ref::<T>().expect("use_state type mismatch (called with a different type than the previous render)").clone()
    });
    (value, StateSetter { id, index, _marker: std::marker::PhantomData })
}

/// テスト・ツール専用: 指定インスタンスのフック状態を破棄する
/// (コンポーネントがツリーから外れた際の後始末に相当、次段階で
/// アプリのアンマウント処理から呼ぶ想定)。
pub fn drop_instance(id: ComponentId) {
    HOOK_STORE.with(|store| {
        store.borrow_mut().remove(&id);
    });
    DIRTY.with(|dirty| {
        dirty.borrow_mut().remove(&id);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_state_returns_initial_value_on_first_render() {
        let id = 1;
        let value = run_component(id, || {
            let (count, _set) = use_state(|| 0i32);
            count
        });
        assert_eq!(value, 0);
        drop_instance(id);
    }

    #[test]
    fn setter_updates_value_seen_on_next_render() {
        let id = 2;
        let setter = run_component(id, || {
            let (_count, set) = use_state(|| 0i32);
            set
        });
        setter.set(5);
        let value = run_component(id, || {
            let (count, _set) = use_state(|| 0i32);
            count
        });
        assert_eq!(value, 5);
        drop_instance(id);
    }

    #[test]
    fn update_reads_previous_value() {
        let id = 3;
        let setter = run_component(id, || {
            let (_count, set) = use_state(|| 10i32);
            set
        });
        setter.update(|prev| prev + 1);
        let value = run_component(id, || {
            let (count, _set) = use_state(|| 10i32);
            count
        });
        assert_eq!(value, 11);
        drop_instance(id);
    }

    #[test]
    fn setter_marks_component_dirty_and_take_dirty_consumes_it() {
        let id = 4;
        let setter = run_component(id, || {
            let (_count, set) = use_state(|| 0i32);
            set
        });
        assert!(!take_dirty(id));
        setter.set(1);
        assert!(take_dirty(id));
        assert!(!take_dirty(id), "take_dirty must consume the flag");
        drop_instance(id);
    }

    #[test]
    fn multiple_hooks_in_one_component_track_independent_slots() {
        let id = 5;
        let (name_setter, age_setter) = run_component(id, || {
            let (_name, set_name) = use_state(|| "alice".to_string());
            let (_age, set_age) = use_state(|| 30i32);
            (set_name, set_age)
        });
        age_setter.set(31);
        let (name, age) = run_component(id, || {
            let (name, _) = use_state(|| "alice".to_string());
            let (age, _) = use_state(|| 30i32);
            (name, age)
        });
        assert_eq!(name, "alice");
        assert_eq!(age, 31);
        name_setter.set("bob".to_string());
        drop_instance(id);
    }

    #[test]
    fn different_component_instances_have_independent_state() {
        let id_a = 6;
        let id_b = 7;
        let setter_a = run_component(id_a, || {
            let (_count, set) = use_state(|| 100i32);
            set
        });
        setter_a.set(200);
        let value_b = run_component(id_b, || {
            let (count, _set) = use_state(|| 100i32);
            count
        });
        assert_eq!(value_b, 100, "instance b must not see instance a's update");
        drop_instance(id_a);
        drop_instance(id_b);
    }
}
