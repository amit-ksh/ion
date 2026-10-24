//! Pure reporting-line edits, shared by graph and keyboard member controls.
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub(super) fn parent_allowed<T: Copy + Eq + Hash>(
    parents: &HashMap<T, Option<T>>,
    id: T,
    parent: T,
) -> bool {
    if !parents.contains_key(&id) {
        return false;
    }
    let mut current = Some(parent);
    let mut seen = HashSet::new();
    while let Some(candidate) = current {
        if candidate == id || !seen.insert(candidate) {
            return false;
        }
        let Some(next) = parents.get(&candidate) else {
            return false;
        };
        current = *next;
    }
    true
}

pub(super) fn reparent<T: Copy + Eq + Hash>(
    members: &mut [(T, Option<T>)],
    id: T,
    parent: Option<T>,
) -> Result<(), &'static str> {
    let parents = members.iter().copied().collect();
    if !members.iter().any(|m| m.0 == id) {
        return Err("This agent is no longer in the team.");
    }
    if parent.is_some_and(|parent| !parent_allowed(&parents, id, parent)) {
        return Err("Reporting lines cannot contain a cycle.");
    }
    for member in members {
        if member.0 == id {
            member.1 = parent;
        } else if parent.is_none() && member.1.is_none() {
            member.1 = Some(id);
        }
    }
    Ok(())
}

pub(super) fn remove<T: Copy + Eq + Hash>(members: &mut Vec<(T, Option<T>)>, id: T) {
    let Some(removed) = members.iter().find(|m| m.0 == id).copied() else {
        return;
    };
    members.retain(|m| m.0 != id);
    let replacement = removed.1.or_else(|| {
        members
            .iter()
            .find(|m| m.1 == Some(id))
            .or_else(|| members.first())
            .map(|m| m.0)
    });
    for member in members {
        if removed.1.is_none() && Some(member.0) == replacement {
            member.1 = None;
        } else if member.1 == Some(id) {
            member.1 = replacement;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cannot_move_manager_under_a_descendant_or_missing_agent() {
        let mut members = vec![(0, None), (1, Some(0)), (2, Some(1))];
        let original = members.clone();
        assert!(reparent(&mut members, 0, Some(2)).is_err());
        assert!(reparent(&mut members, 1, Some(9)).is_err());
        assert_eq!(members, original);
    }
    #[test]
    fn promoting_a_descendant_retains_all_members_with_one_lead() {
        let mut members = vec![(0, None), (1, Some(0)), (2, Some(1)), (3, Some(0))];
        reparent(&mut members, 2, None).unwrap();
        assert_eq!(
            members,
            vec![(0, Some(2)), (1, Some(0)), (2, None), (3, Some(0))]
        );
        assert_eq!(members.iter().filter(|m| m.1.is_none()).count(), 1);
    }
    #[test]
    fn removing_a_manager_preserves_and_reparents_its_direct_reports() {
        let mut members = vec![(0, None), (1, Some(0)), (2, Some(1)), (3, Some(1))];
        remove(&mut members, 1);
        assert_eq!(members, vec![(0, None), (2, Some(0)), (3, Some(0))]);
    }
    #[test]
    fn removing_lead_promotes_one_direct_report_and_preserves_deeper_branches() {
        let mut members = vec![(0, None), (1, Some(0)), (2, Some(0)), (3, Some(2))];
        remove(&mut members, 0);
        assert_eq!(members, vec![(1, None), (2, Some(1)), (3, Some(2))]);
    }
    #[test]
    fn stale_removal_is_noop_and_only_member_can_be_removed() {
        let mut members = vec![(0, None)];
        remove(&mut members, 9);
        assert_eq!(members, vec![(0, None)]);
        remove(&mut members, 0);
        assert!(members.is_empty());
    }
}
