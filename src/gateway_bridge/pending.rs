//! Window-owned, pairing-scoped outbound state. This module performs no I/O.
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

struct Entry<V> {
    owners: HashSet<u64>,
    value: V,
    terminal: bool,
}

pub(super) struct Pending<K, V> {
    bindings: HashMap<u64, String>,
    entries: HashMap<(String, K), Entry<V>>,
}

impl<K, V> Default for Pending<K, V> {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
            entries: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash, V> Pending<K, V> {
    /// Refreshing the same binding must not erase outstanding resolutions.
    /// Changing one window's binding cannot discard another window's state.
    pub fn bind(&mut self, owner: u64, scope: Option<String>) {
        if self.bindings.get(&owner) == scope.as_ref() {
            return;
        }
        self.entries.retain(|_, entry| {
            entry.owners.remove(&owner);
            !entry.owners.is_empty()
        });
        if let Some(scope) = scope {
            self.bindings.insert(owner, scope);
        } else {
            self.bindings.remove(&owner);
        }
    }

    pub fn insert(&mut self, owner: u64, scope: String, key: K, value: V, terminal: bool) {
        if self.bindings.get(&owner) != Some(&scope) {
            return;
        }
        match self.entries.entry((scope, key)) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Entry {
                    owners: HashSet::from([owner]),
                    value,
                    terminal,
                });
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let entry = entry.get_mut();
                entry.owners.insert(owner);
                // A stale window's pending view cannot reopen a queued resolution.
                if !entry.terminal || terminal {
                    entry.value = value;
                    entry.terminal = terminal;
                }
            }
        }
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.entries.keys().any(|(candidate, _)| candidate == scope)
    }

    /// Only matching credentials may publish. A newer pairing's queued events
    /// survive a worker iteration that still holds the previous API snapshot.
    pub fn retain_scope(&mut self, scope: &str, mut retain: impl FnMut(&V) -> bool) {
        self.entries
            .retain(|(candidate, _), entry| candidate != scope || retain(&entry.value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refreshing_or_rebinding_one_window_preserves_another_windows_approval() {
        let mut queue = Pending::default();
        queue.bind(1, Some("account-a".into()));
        queue.bind(2, Some("account-a".into()));
        queue.insert(1, "account-a".into(), "approval-a", "approve A", false);
        queue.bind(2, Some("account-a".into()));
        queue.insert(2, "account-a".into(), "approval-b", "approve B", false);
        queue.bind(2, Some("account-b".into()));
        let mut sent = Vec::new();
        queue.retain_scope("account-a", |value| {
            sent.push(*value);
            false
        });
        assert_eq!(sent, ["approve A"]);
    }

    #[test]
    fn new_pairing_events_survive_an_old_worker_snapshot_without_cross_account_delivery() {
        let mut queue = Pending::default();
        queue.bind(1, Some("old-pairing".into()));
        queue.insert(1, "old-pairing".into(), "approval", "old", false);
        queue.bind(1, Some("new-pairing".into()));
        queue.insert(1, "new-pairing".into(), "approval", "new", false);
        queue.insert(1, "old-pairing".into(), "late-old", "stale", false);
        queue.retain_scope("old-pairing", |_| {
            panic!("old credentials saw a new/retired event")
        });
        assert!(queue.has_scope("new-pairing"));
        let mut sent = Vec::new();
        queue.retain_scope("new-pairing", |value| {
            sent.push(*value);
            false
        });
        assert_eq!(sent, ["new"]);
    }

    #[test]
    fn windows_coalesce_and_resolve_wins_over_stale_republication() {
        let mut queue = Pending::default();
        for owner in [1, 2] {
            queue.bind(owner, Some("pairing".into()));
        }
        queue.insert(1, "pairing".into(), "approval", "pending", false);
        queue.insert(2, "pairing".into(), "approval", "resolved", true);
        queue.bind(1, Some("pairing".into()));
        queue.insert(1, "pairing".into(), "approval", "stale pending", false);
        let mut attempts = Vec::new();
        queue.retain_scope("pairing", |value| {
            attempts.push(*value);
            true
        });
        assert_eq!(attempts, ["resolved"]);
        // A failed send is retained; one successful send retires every copy.
        queue.retain_scope("pairing", |value| {
            assert_eq!(*value, "resolved");
            false
        });
        queue.retain_scope("pairing", |_| panic!("duplicate dispatch"));
    }

    #[test]
    fn identical_keys_in_other_accounts_do_not_overwrite_or_wake_current_account() {
        let mut queue = Pending::default();
        queue.bind(1, Some("a".into()));
        queue.bind(2, Some("b".into()));
        queue.insert(1, "a".into(), "same-key", "A", true);
        queue.insert(2, "b".into(), "same-key", "B", false);
        queue.retain_scope("a", |value| {
            assert_eq!(*value, "A");
            false
        });
        assert!(!queue.has_scope("a"));
        assert!(queue.has_scope("b"));
        queue.retain_scope("b", |value| {
            assert_eq!(*value, "B");
            false
        });
    }

    #[test]
    fn backlog_is_not_silently_cleared_at_one_thousand_updates() {
        let mut queue = Pending::default();
        queue.bind(1, Some("a".into()));
        for key in 0..1_001 {
            queue.insert(1, "a".into(), key, key, false);
        }
        let mut count = 0;
        queue.retain_scope("a", |_| {
            count += 1;
            false
        });
        assert_eq!(count, 1_001);
        assert!(!queue.has_scope("a"));
    }
}
