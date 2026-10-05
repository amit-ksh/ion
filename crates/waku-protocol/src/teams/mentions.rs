//! Explicit team addressing shared by all clients and the daemon.
use crate::control::Team;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamInvocation {
    pub team_id: Uuid,
    pub objective: String,
}

/// Team addressing is reserved at the start of a new task, just like a recipient.
/// Unknown bare @tokens remain file mentions; qualified @team: tokens fail closed.
/// Quotes, code, later mentions and ordinary team names never select execution.
pub fn resolve(prompt: &str, teams: &[Team]) -> Result<Option<TeamInvocation>, String> {
    let text = prompt.trim_start();
    let Some(address) = text.strip_prefix('@') else {
        return Ok(None);
    };
    let (qualified, address) = match address.strip_prefix("team:") {
        Some(address) => (true, address),
        None => (false, address),
    };
    let (name, rest) = if let Some(braced) = address.strip_prefix('{') {
        let Some(end) = braced.find('}') else {
            return Err("Close the team name with } before sending.".into());
        };
        (&braced[..end], &braced[end + 1..])
    } else {
        let end = address.find(char::is_whitespace).unwrap_or(address.len());
        (&address[..end], &address[end..])
    };
    // Preserve explicit path mentions even when a saved team has a path-like
    // name. The picker can address those teams with the qualified form.
    if !qualified && !address.starts_with('{') && name.contains(['/', '\\']) {
        return Ok(None);
    }
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return if qualified {
            Err("Add a space after the team mention.".into())
        } else {
            Ok(None)
        };
    }
    let id = qualified.then(|| Uuid::parse_str(name).ok()).flatten();
    let found: Vec<_> = teams
        .iter()
        .filter(|t| {
            if let Some(id) = id {
                t.id == id
            } else {
                t.name.trim().to_lowercase() == name.to_lowercase()
            }
        })
        .collect();
    match found.as_slice() {
        [] if qualified => {
            Err("This team no longer exists. Choose a team from @ suggestions.".into())
        }
        [] => Ok(None),
        [team] => {
            let objective = rest.trim();
            if objective.is_empty() {
                return Err("Describe the task after the team mention.".into());
            }
            Ok(Some(TeamInvocation {
                team_id: team.id,
                objective: objective.to_owned(),
            }))
        }
        _ => Err("Several teams share this name. Choose the team from @ suggestions.".into()),
    }
}

/// Readable names where unambiguous; UUID addressing survives duplicates and
/// names containing braces/newlines without inventing or persisting an alias.
pub fn mention(team: &Team, teams: &[Team]) -> String {
    if team.name.contains(['{', '}', '\n', '\r'])
        || teams
            .iter()
            .filter(|t| t.name.trim().to_lowercase() == team.name.trim().to_lowercase())
            .count()
            != 1
    {
        format!("@team:{}", team.id)
    } else {
        format!("@team:{{{}}}", team.name.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn team(name: &str) -> Team {
        Team {
            id: Uuid::new_v4(),
            name: name.into(),
            objective: "Work".into(),
            members: vec![],
        }
    }
    #[test]
    fn only_explicit_leading_addresses_select_a_team() {
        let teams = vec![
            team("engineering"),
            team("Design team"),
            team("src/main.rs"),
            team("./engineering"),
        ];
        for text in [
            "Fix engineering",
            "Ask @engineering to help",
            "`@engineering` fix",
            "\"@engineering fix\"",
            "@src/main.rs inspect",
            "@./engineering inspect",
        ] {
            assert_eq!(resolve(text, &teams), Ok(None), "{text}");
        }
        let call = resolve("  @engineering fix this\nwith tests", &teams)
            .unwrap()
            .unwrap();
        assert_eq!(call.team_id, teams[0].id);
        assert_eq!(call.objective, "fix this\nwith tests");
        assert_eq!(
            resolve("@{Design team} sketch", &teams)
                .unwrap()
                .unwrap()
                .team_id,
            teams[1].id
        );
        assert_eq!(
            resolve("@ENGINEERING fix", &teams)
                .unwrap()
                .unwrap()
                .team_id,
            teams[0].id
        );
    }
    #[test]
    fn duplicate_deleted_and_incomplete_addresses_cannot_run_solo() {
        let teams = vec![team("same"), team("SAME")];
        assert!(resolve("@same fix", &teams).is_err());
        assert!(resolve("@team:deleted fix", &teams).is_err());
        assert!(resolve("@{same", &teams).is_err());
        assert!(resolve("@team:{same}fix", &teams).is_err());
        assert!(resolve("@team:{same}", &teams).is_err());
        let address = mention(&teams[0], &teams);
        assert_eq!(
            resolve(&format!("{address} fix"), &teams)
                .unwrap()
                .unwrap()
                .team_id,
            teams[0].id
        );
    }
    #[test]
    fn suggestions_round_trip_spaced_unicode_and_unusual_names() {
        for name in [
            "Engineering",
            "Design team",
            "équipe",
            "Name }",
            "Team\nName",
        ] {
            let teams = vec![team(name)];
            let call = resolve(&format!("{} build", mention(&teams[0], &teams)), &teams)
                .unwrap()
                .unwrap();
            assert_eq!(call.team_id, teams[0].id);
        }
    }
}
