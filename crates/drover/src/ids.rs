pub fn qualify(id: &str, machine: Option<&str>) -> String {
    machine.map_or_else(|| id.to_owned(), |m| format!("{m}/{id}"))
}

pub fn local<'a>(id: &'a str, machine: Option<&str>) -> &'a str {
    machine
        .and_then(|m| id.strip_prefix(m).and_then(|rest| rest.strip_prefix('/')))
        .unwrap_or(id)
}

pub fn split(key: &str) -> (Option<&str>, &str) {
    match key.split_once('/') {
        Some((machine, id)) => (Some(machine), id),
        None => (None, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ids_round_trip_through_qualify() {
        assert_eq!(qualify("t1", None), "t1");
        assert_eq!(qualify("t1", Some("mac")), "mac/t1");
        assert_eq!(local("mac/t1", Some("mac")), "t1");
        assert_eq!(local("t1", Some("mac")), "t1");
        assert_eq!(local("t1", None), "t1");
        assert_eq!(split("mac/sp1"), (Some("mac"), "sp1"));
        assert_eq!(split("sp1"), (None, "sp1"));
    }
}
