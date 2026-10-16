use std::collections::{HashMap, HashSet};

use crate::state::Session;
use crate::store::UNKNOWN_ID_PREFIX;

pub const MAX_ORDER_LEN: usize = 500;

pub fn arrange<'a>(sessions: &[&'a Session], order: &[String]) -> (Vec<&'a Session>, Vec<String>) {
    let (unknown, draggable): (Vec<&Session>, Vec<&Session>) = sessions
        .iter()
        .copied()
        .partition(|s| s.id.starts_with(UNKNOWN_ID_PREFIX));
    let by_id: HashMap<&str, &Session> = draggable.iter().map(|s| (s.id.as_str(), *s)).collect();
    let mut next: Vec<String> = order.to_vec();
    for session in &draggable {
        if next.iter().any(|id| id == &session.id) {
            continue;
        }
        let after_same_project = (!session.cwd.is_empty())
            .then(|| {
                next.iter()
                    .rposition(|id| by_id.get(id.as_str()).is_some_and(|s| s.cwd == session.cwd))
            })
            .flatten();
        match after_same_project {
            Some(index) => next.insert(index + 1, session.id.clone()),
            None => next.push(session.id.clone()),
        }
    }
    trim_hidden(&mut next, &by_id.keys().copied().collect());
    let mut arranged: Vec<&Session> = next.iter().filter_map(|id| by_id.get(id.as_str()).copied()).collect();
    arranged.extend(unknown);
    (arranged, next)
}

// いつ加えたかは覚えていないので、古さではなく位置で決める
fn trim_hidden(order: &mut Vec<String>, shown: &HashSet<&str>) {
    let mut excess = order.len().saturating_sub(MAX_ORDER_LEN);
    order.retain(|id| {
        if excess > 0 && !shown.contains(id.as_str()) {
            excess -= 1;
            return false;
        }
        true
    });
}

pub fn move_before(order: &[String], id: &str, before: Option<&str>) -> Vec<String> {
    if before == Some(id) {
        return order.to_vec();
    }
    let mut next: Vec<String> = order
        .iter()
        .filter(|existing| existing.as_str() != id)
        .cloned()
        .collect();
    let index = before
        .and_then(|target| next.iter().position(|existing| existing == target))
        .unwrap_or(next.len());
    next.insert(index, id.to_string());
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::ev;
    use crate::store::SessionStore;

    fn sessions(specs: &[(&str, &str)]) -> Vec<Session> {
        let mut store = SessionStore::default();
        for (i, (id, cwd)) in specs.iter().enumerate() {
            let mut e = ev("SessionStart", id, i as u64 + 1);
            e.cwd = cwd.to_string();
            store.apply(&e);
        }
        store.sessions().to_vec()
    }

    fn ids(arranged: &[&Session]) -> Vec<String> {
        arranged.iter().map(|s| s.id.clone()).collect()
    }

    #[test]
    fn follows_saved_order_and_keeps_hidden_entries() {
        let all = sessions(&[("a", "/w/api"), ("b", "/w/web")]);
        let refs: Vec<&Session> = all.iter().collect();
        let order = vec!["gone".to_string(), "b".into(), "a".into()];
        let (arranged, next) = arrange(&refs, &order);
        assert_eq!(ids(&arranged), ["b", "a"]);
        assert_eq!(next, order, "表示していない gone も後の再開のために残す");
    }

    #[test]
    fn new_session_goes_after_last_session_of_same_cwd() {
        let all = sessions(&[("a", "/w/api"), ("b", "/w/web"), ("c", "/w/api")]);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, next) = arrange(&refs, &["a".into(), "b".into()]);
        assert_eq!(ids(&arranged), ["a", "c", "b"]);
        assert_eq!(next, ["a", "c", "b"]);
    }

    #[test]
    fn new_session_of_new_project_goes_last() {
        let all = sessions(&[("a", "/w/api"), ("b", "/w/web")]);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, _) = arrange(&refs, &["a".into()]);
        assert_eq!(ids(&arranged), ["a", "b"]);
    }

    #[test]
    fn session_with_empty_cwd_does_not_join_others() {
        let all = sessions(&[("a", ""), ("b", "/w/web"), ("c", "")]);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, _) = arrange(&refs, &["a".into(), "b".into()]);
        assert_eq!(ids(&arranged), ["a", "b", "c"]);
    }

    #[test]
    fn unknown_process_rows_stay_last_and_are_not_saved() {
        let mut all = sessions(&[("a", "/w/api")]);
        let mut unknown = all[0].clone();
        unknown.id = format!("{UNKNOWN_ID_PREFIX}42");
        all.insert(0, unknown);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, next) = arrange(&refs, &[]);
        assert_eq!(ids(&arranged), ["a".to_string(), format!("{UNKNOWN_ID_PREFIX}42")]);
        assert_eq!(next, ["a"]);
    }

    #[test]
    fn trims_hidden_entries_from_the_front_beyond_the_limit() {
        let all = sessions(&[("shown", "/w/api")]);
        let refs: Vec<&Session> = all.iter().collect();
        let mut order: Vec<String> = (0..MAX_ORDER_LEN).map(|i| format!("old{i}")).collect();
        order.insert(0, "shown".into());
        let (_, next) = arrange(&refs, &order);
        assert_eq!(next.len(), MAX_ORDER_LEN);
        assert_eq!(next[0], "shown", "表示中のものは消さない");
        assert_eq!(next[1], "old1", "表示していないものは先頭側から消す");
    }

    #[test]
    fn moves_before_target_or_to_end() {
        let order: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        assert_eq!(move_before(&order, "c", Some("a")), ["c", "a", "b"]);
        assert_eq!(move_before(&order, "a", None), ["b", "c", "a"]);
        assert_eq!(
            move_before(&order, "x", Some("b")),
            ["a", "x", "b", "c"],
            "並びにない ID も入れられる"
        );
        assert_eq!(
            move_before(&order, "a", Some("a")),
            order,
            "自分の前に置いても変わらない"
        );
    }
}
