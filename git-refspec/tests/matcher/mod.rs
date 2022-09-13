use git_refspec::instruction::Fetch;

mod match_ {
    use git_refspec::Match;

    #[test]
    fn default_is_not_matched() {
        assert!(!Match::default().matched())
    }
}

#[test]
fn fetch_only() {
    baseline::parse_input().unwrap();
    baseline::parse().unwrap();
    let _spec = Fetch::Only {
        src: "refs/heads/main".into(),
    };
}

mod baseline {
    use bstr::{BString, ByteSlice, ByteVec};
    use git_hash::ObjectId;
    use std::collections::HashMap;

    #[derive(Debug)]
    pub struct Ref {
        pub name: BString,
        pub target: ObjectId,
        /// Set if this is a tag, pointing to the tag object itself
        pub tag: Option<ObjectId>,
    }

    #[derive(Debug)]
    pub struct Mapping {
        pub remote: BString,
        /// `None` if there is no destination/tracking branch
        pub local: Option<BString>,
    }

    type Baseline = HashMap<Vec<BString>, Result<Vec<Mapping>, BString>>;

    pub fn parse_input() -> crate::Result<Vec<Ref>> {
        let dir = git_testtools::scripted_fixture_repo_read_only("match_baseline.sh")?;
        let refs_buf = std::fs::read(dir.join("clone").join("remote-refs.list"))?;
        let mut out = Vec::new();
        for line in refs_buf.lines() {
            if line.starts_with(b"From ") {
                continue;
            }
            let mut tokens = line.splitn(2, |b| *b == b'\t');
            let target = ObjectId::from_hex(tokens.next().expect("hex-sha"))?;
            let name = tokens.next().expect("name");
            if !name.ends_with(b"^{}") {
                out.push(Ref {
                    name: name.into(),
                    target,
                    tag: None,
                })
            } else {
                let last = out.last_mut().unwrap();
                let tag = last.target;
                last.target = target;
                last.tag = Some(tag);
            }
        }
        Ok(out)
    }

    pub fn parse() -> crate::Result<Baseline> {
        let dir = git_testtools::scripted_fixture_repo_read_only("match_baseline.sh")?;
        let buf = std::fs::read(dir.join("clone").join("baseline.git"))?;

        let mut map = HashMap::new();
        let mut mappings = Vec::new();
        let mut fatal = None;
        for line in buf.lines() {
            if line.starts_with(b"From ") {
                continue;
            }
            match line.strip_prefix(b"specs: ") {
                Some(specs) => {
                    let key: Vec<_> = specs.split(|b| *b == b' ').map(BString::from).collect();
                    let value = match fatal.take() {
                        Some(message) => Err(message),
                        None => Ok(std::mem::take(&mut mappings)),
                    };
                    map.insert(key, value);
                }
                None => match line.strip_prefix(b"fatal: ") {
                    Some(message) => {
                        fatal = Some(message.into());
                    }
                    None => {
                        let past_note = line
                            .splitn(2, |b| *b == b']')
                            .nth(1)
                            .or_else(|| line.strip_prefix(b" * branch "))
                            .unwrap();
                        let mut tokens = past_note.split(|b| *b == b' ').filter(|t| !t.is_empty());

                        let lhs = tokens.next().unwrap().trim();
                        tokens.next();
                        let rhs = tokens.next().unwrap().trim();
                        mappings.push(Mapping {
                            remote: full_remote_ref(lhs.into()),
                            local: (rhs != b"FETCH_HEAD").then(|| full_tracking_ref(rhs.into())),
                        })
                    }
                },
            }
        }

        Ok(map)
    }

    fn full_remote_ref(mut name: BString) -> BString {
        if !name.contains(&b'/') {
            name.insert_str(0, b"refs/heads/");
        }
        name
    }

    fn full_tracking_ref(mut name: BString) -> BString {
        if name.starts_with_str(b"origin/") {
            name.insert_str(0, b"refs/remotes/");
        }
        name
    }
}
