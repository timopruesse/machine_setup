use super::message::{CatalogEffect, CatalogInput, CatalogMessage};
use super::model::CatalogMode;
use super::state::CatalogState;

pub fn reduce(mut state: CatalogState, msg: CatalogMessage) -> (CatalogState, CatalogEffect) {
    let CatalogMessage::Input(input) = msg;
    match input {
        CatalogInput::Quit => {
            let effect = match state.mode {
                CatalogMode::Browse => CatalogEffect::Quit,
                CatalogMode::Select => CatalogEffect::Abort,
            };
            (state, effect)
        }
        CatalogInput::Abort => (state, CatalogEffect::Abort),
        CatalogInput::ClearFilterOrLeave => {
            if state.filter_active() {
                state.search_mode = false;
                state.search_query.clear();
                state.refresh_filter();
                (state, CatalogEffect::None)
            } else {
                let effect = match state.mode {
                    CatalogMode::Browse => CatalogEffect::Quit,
                    CatalogMode::Select => CatalogEffect::Abort,
                };
                (state, effect)
            }
        }
        CatalogInput::EnterSearch => {
            state.search_mode = true;
            (state, CatalogEffect::None)
        }
        CatalogInput::ExitSearch => {
            state.search_mode = false;
            state.search_query.clear();
            state.refresh_filter();
            (state, CatalogEffect::None)
        }
        CatalogInput::ConfirmSearch => {
            state.search_mode = false;
            state.refresh_filter();
            (state, CatalogEffect::None)
        }
        CatalogInput::SearchChar(c) => {
            if state.search_mode {
                state.search_query.push(c);
                state.refresh_filter();
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::SearchBackspace => {
            if state.search_mode {
                state.search_query.pop();
                state.refresh_filter();
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::SelectNext | CatalogInput::SelectPrev => {
            if state.filtered_indices.is_empty() {
                return (state, CatalogEffect::None);
            }
            let go_next = matches!(input, CatalogInput::SelectNext);
            let pos = state
                .filtered_indices
                .iter()
                .position(|&i| i == state.selected)
                .unwrap_or(0);
            let next = if go_next {
                (pos + 1) % state.filtered_indices.len()
            } else if pos == 0 {
                state.filtered_indices.len() - 1
            } else {
                pos - 1
            };
            state.selected = state.filtered_indices[next];
            (state, CatalogEffect::None)
        }
        CatalogInput::ToggleCheck => {
            if matches!(state.mode, CatalogMode::Select)
                && state.filtered_indices.contains(&state.selected)
                && !state.checked.remove(&state.selected)
            {
                state.checked.insert(state.selected);
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::SelectAllVisible => {
            if matches!(state.mode, CatalogMode::Select) {
                for &i in &state.filtered_indices {
                    state.checked.insert(i);
                }
            }
            (state, CatalogEffect::None)
        }
        CatalogInput::ConfirmSelection => {
            if !matches!(state.mode, CatalogMode::Select) {
                return (state, CatalogEffect::None);
            }
            if state.checked.is_empty() {
                return (state, CatalogEffect::Abort);
            }
            let ids: Vec<String> = state
                .checked
                .iter()
                .filter_map(|&i| state.items.get(i).map(|it| it.id.clone()))
                .collect();
            (state, CatalogEffect::Confirm(ids))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{CatalogItem, CatalogStatus};
    use super::*;

    fn item(id: &str) -> CatalogItem {
        CatalogItem {
            id: id.into(),
            title: id.into(),
            status: CatalogStatus::NotInstalled,
            os_label: "all".into(),
            installed_at: "-".into(),
            updated_at: "-".into(),
            badges: vec![],
            detail: vec![],
        }
    }

    fn browse(ids: &[&str]) -> CatalogState {
        CatalogState::new(ids.iter().map(|s| item(s)).collect(), CatalogMode::Browse)
    }

    fn select_mode(ids: &[&str]) -> CatalogState {
        CatalogState::new(ids.iter().map(|s| item(s)).collect(), CatalogMode::Select)
    }

    #[test]
    fn quit_returns_quit_in_browse() {
        let (s, e) = reduce(browse(&["a"]), CatalogMessage::Input(CatalogInput::Quit));
        assert_eq!(e, CatalogEffect::Quit);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn quit_returns_abort_in_select() {
        let (_, e) = reduce(
            select_mode(&["a"]),
            CatalogMessage::Input(CatalogInput::Quit),
        );
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn abort_returns_abort_in_select() {
        let (_, e) = reduce(
            select_mode(&["a"]),
            CatalogMessage::Input(CatalogInput::Abort),
        );
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn esc_clears_filter_without_leaving() {
        let state = browse(&["alpha", "beta"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::EnterSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SearchChar('a')));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSearch));
        assert!(state.filter_active());
        let (state, e) = reduce(
            state,
            CatalogMessage::Input(CatalogInput::ClearFilterOrLeave),
        );
        assert_eq!(e, CatalogEffect::None);
        assert!(!state.filter_active());
        assert_eq!(state.filtered_indices.len(), 2);
    }

    #[test]
    fn esc_leaves_when_no_filter() {
        let (_, e) = reduce(
            browse(&["a"]),
            CatalogMessage::Input(CatalogInput::ClearFilterOrLeave),
        );
        assert_eq!(e, CatalogEffect::Quit);
        let (_, e) = reduce(
            select_mode(&["a"]),
            CatalogMessage::Input(CatalogInput::ClearFilterOrLeave),
        );
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn navigate_stays_within_filtered_set() {
        let state = browse(&["alpha", "beta", "other"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::EnterSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SearchChar('a')));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSearch));
        assert_eq!(state.filtered_indices, vec![0, 1]);
        let before = state.selected;
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SelectNext));
        assert_ne!(state.selected, before);
        assert!(state.filtered_indices.contains(&state.selected));
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn toggle_check_only_in_select_mode() {
        let (state, _) = reduce(
            browse(&["a"]),
            CatalogMessage::Input(CatalogInput::ToggleCheck),
        );
        assert!(state.checked.is_empty());

        let (state, _) = reduce(
            select_mode(&["a", "b"]),
            CatalogMessage::Input(CatalogInput::ToggleCheck),
        );
        assert!(state.checked.contains(&0));
    }

    #[test]
    fn select_all_visible_checks_filtered_only() {
        let state = select_mode(&["alpha", "beta", "gamma"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::EnterSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SearchChar('p')));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSearch));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SelectAllVisible));
        for &i in &state.filtered_indices {
            assert!(state.checked.contains(&i));
        }
        let beta = state.items.iter().position(|it| it.id == "beta").unwrap();
        assert!(!state.checked.contains(&beta));
    }

    #[test]
    fn confirm_with_checks_returns_ids() {
        let state = select_mode(&["a", "b"]);
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ToggleCheck));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::SelectNext));
        let (state, _) = reduce(state, CatalogMessage::Input(CatalogInput::ToggleCheck));
        let (_, e) = reduce(state, CatalogMessage::Input(CatalogInput::ConfirmSelection));
        match e {
            CatalogEffect::Confirm(ids) => assert_eq!(ids, vec!["a".to_string(), "b".to_string()]),
            other => panic!("expected Confirm, got {other:?}"),
        }
    }

    #[test]
    fn confirm_with_zero_checks_aborts() {
        let (_, e) = reduce(
            select_mode(&["a"]),
            CatalogMessage::Input(CatalogInput::ConfirmSelection),
        );
        assert_eq!(e, CatalogEffect::Abort);
    }

    #[test]
    fn confirm_ignored_in_browse() {
        let (_, e) = reduce(
            browse(&["a"]),
            CatalogMessage::Input(CatalogInput::ConfirmSelection),
        );
        assert_eq!(e, CatalogEffect::None);
    }
}
