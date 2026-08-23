use std::{cell::Cell, rc::Rc};

use gpui::{
    App, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    TestAppContext, VisualTestContext, Window,
    accesskit::{Action, ActionRequest, TreeId},
    point, px,
};
use gpui_component::{
    IndexPath, Root,
    list::{List, ListDelegate, ListItem, ListState},
};

#[derive(Debug)]
struct Delegate {
    selected: Option<IndexPath>,
    confirmed_row: Rc<Cell<Option<usize>>>,
    item_order: Vec<usize>,
}

struct ListView(Entity<ListState<Delegate>>);

impl Render for ListView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        List::new(&self.0)
    }
}

impl ListDelegate for Delegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.item_order.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.item_order[ix.row];
        Some(ListItem::new(ix.row).child(format!("Item {}", item + 1)))
    }

    fn item_accessibility_id(&self, ix: IndexPath, _: &App) -> Option<gpui::SharedString> {
        Some(format!("test.list-item.{}", self.item_order[ix.row]).into())
    }

    fn item_accessibility_label(&self, ix: IndexPath, _: &App) -> Option<gpui::SharedString> {
        Some(format!("Item {}", self.item_order[ix.row] + 1).into())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }

    fn confirm(&mut self, _: bool, _: &mut Window, _: &mut Context<ListState<Self>>) {
        self.confirmed_row.set(self.selected.map(|ix| ix.row));
    }
}

fn list_window(
    cx: &mut TestAppContext,
) -> (
    gpui::WindowHandle<Root>,
    gpui::Entity<ListState<Delegate>>,
    Rc<Cell<Option<usize>>>,
) {
    cx.update(gpui_component::init);
    let confirmed_row = Rc::new(Cell::new(None));
    let mut state = None;
    let window = cx.add_window(|window, cx| {
        let list = cx.new(|cx| {
            ListState::new(
                Delegate {
                    selected: None,
                    confirmed_row: confirmed_row.clone(),
                    item_order: (0..100).collect(),
                },
                window,
                cx,
            )
        });
        state = Some(list.clone());
        let view = cx.new(|_| ListView(list.clone()));
        Root::new(view, window, cx)
    });
    (window, state.expect("list state"), confirmed_row)
}

#[gpui::test]
fn list_accessibility_click_uses_the_confirm_path(cx: &mut TestAppContext) {
    let (window, state, confirmed_row) = list_window(cx);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.activate_accessibility();
    let node_id = visual
        .accessibility_node_id("test.list-item.1")
        .expect("second accessible list item");

    visual.simulate_accessibility_action(ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: node_id,
        data: None,
    });

    assert_eq!(confirmed_row.get(), Some(1));
    assert_eq!(
        state.read_with(&visual.cx, |state, _| state.selected_index()),
        Some(IndexPath::new(1))
    );
}

#[gpui::test]
fn list_return_context_restores_selection_scroll_and_focus(cx: &mut TestAppContext) {
    let (window, state, _) = list_window(cx);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(1)), window, cx);
            state
                .scroll_handle()
                .base_handle()
                .set_offset(point(px(0.), px(-42.)));
        });
    });
    let return_context = state.read_with(&visual.cx, |state, cx| state.return_context(cx));
    visual.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(2)), window, cx);
            state
                .scroll_handle()
                .base_handle()
                .set_offset(point(px(0.), px(-100.)));
            state.restore_return_context(return_context, window, cx);
        });
        assert!(state.focus_handle(cx).is_focused(window));
    });

    assert_eq!(
        state.read_with(&visual.cx, |state, _| state.selected_index()),
        Some(IndexPath::new(1))
    );
    assert_eq!(
        state.read_with(&visual.cx, |state, _| state
            .scroll_handle()
            .base_handle()
            .offset()),
        point(px(0.), px(-42.))
    );
}

#[gpui::test]
fn list_return_context_restores_stable_item_after_reorder(cx: &mut TestAppContext) {
    let (window, state, _) = list_window(cx);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(1)), window, cx);
        });
    });
    let return_context = state.read_with(&visual.cx, |state, cx| state.return_context(cx));
    assert_eq!(
        return_context.selected_item_id().map(AsRef::as_ref),
        Some("test.list-item.1")
    );

    visual.update(|window, cx| {
        state.update(cx, |state, cx| {
            state.delegate_mut().item_order.swap(1, 2);
            state.restore_return_context(return_context, window, cx);
        });
    });

    assert_eq!(
        state.read_with(&visual.cx, |state, _| state.selected_index()),
        Some(IndexPath::new(2)),
        "return must follow the stable item id instead of the stale index"
    );
}
