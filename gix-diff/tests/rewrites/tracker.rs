use gix_diff::{
    blob::DiffLineStats,
    rewrites,
    rewrites::{
        tracker::{
            visit::{Source, SourceKind},
            ChangeKind,
        },
        Copies, CopySource,
    },
    tree::visit::Action,
    Rewrites,
};
use gix_object::tree::EntryKind;
use pretty_assertions::assert_eq;

use crate::{
    hex_to_id,
    rewrites::{Change, NULL_ID},
};

#[test]
fn rename_by_id() -> crate::Result {
    // Limits are only applied when doing rewrite-checks
    for limit in [0, 1] {
        let rewrites = Rewrites {
            copies: None,
            percentage: None,
            limit,
        };
        let mut track = util::new_tracker(rewrites);
        assert!(
            track.try_push_change(Change::modification(), "a".into()).is_some(),
            "modifications play no role in rename tracking"
        );
        assert!(
            track.try_push_change(Change::deletion(), "b".into()).is_none(),
            "recorded for later matching"
        );
        assert!(
            track.try_push_change(Change::addition(), "c".into()).is_none(),
            "recorded for later matching"
        );
        let mut called = false;
        let out = util::assert_emit(&mut track, |dst, src| {
            assert!(!called, "only one rename pair is expected");
            called = true;
            assert_eq!(
                src.unwrap(),
                Source {
                    entry_mode: EntryKind::Blob.into(),
                    id: NULL_ID,
                    kind: SourceKind::Rename,
                    location: "b".into(),
                    change: &Change::deletion(),
                    diff: None,
                }
            );
            assert_eq!(dst.location, "c");
            Action::Continue
        });
        assert_eq!(
            out,
            rewrites::Outcome {
                options: rewrites,
                ..Default::default()
            },
            "no similarity check was performed, it was all matched by id"
        );
    }
    Ok(())
}

#[test]
fn copy_by_similarity_reports_limit_if_encountered() -> crate::Result {
    let rewrites = Rewrites {
        copies: Some(Copies {
            source: CopySource::FromSetOfModifiedFiles,
            percentage: Some(0.5),
        }),
        percentage: None,
        limit: 1,
    };
    let mut track = util::new_tracker(rewrites);
    let odb = util::add_retained_blobs(
        &mut track,
        [
            (Change::modification(), "a", "a\n"),
            (Change::addition(), "a-cpy-1", "a"),
            (Change::addition(), "a-cpy-2", "a"),
            (Change::modification(), "d", "ab"),
        ],
    );

    let mut calls = 0;
    let out = util::assert_emit_with_objects(
        &mut track,
        |dst, src| {
            assert!(src.is_none());
            match calls {
                0 => assert_eq!(dst.location, "a"),
                1 => assert_eq!(dst.location, "a-cpy-1"),
                2 => assert_eq!(dst.location, "a-cpy-2"),
                3 => assert_eq!(dst.location, "d"),
                _ => panic!("too many emissions"),
            }
            calls += 1;
            Action::Continue
        },
        odb,
    );
    assert_eq!(
        out,
        rewrites::Outcome {
            options: rewrites,
            num_similarity_checks_skipped_for_copy_tracking_due_to_limit: 4,
            ..Default::default()
        },
        "no similarity check was performed at all - all or nothing"
    );
    Ok(())
}

#[test]
fn copy_by_id() -> crate::Result {
    // Limits are only applied when doing rewrite-checks
    for limit in [0, 1] {
        let rewrites = Rewrites {
            copies: Some(Copies {
                source: CopySource::FromSetOfModifiedFiles,
                percentage: None,
            }),
            percentage: None,
            limit,
        };
        let mut track = util::new_tracker(rewrites);
        let odb = util::add_retained_blobs(
            &mut track,
            [
                (Change::modification(), "a", "a"),
                (Change::addition(), "a-cpy-1", "a"),
                (Change::addition(), "a-cpy-2", "a"),
                (Change::modification(), "d", "a"),
            ],
        );

        let mut calls = 0;
        let out = util::assert_emit_with_objects(
            &mut track,
            |dst, src| {
                let id = hex_to_id("2e65efe2a145dda7ee51d1741299f848e5bf752e");
                let source_a = Source {
                    entry_mode: EntryKind::Blob.into(),
                    id: id,
                    kind: SourceKind::Copy,
                    location: "a".into(),
                    change: &Change {
                        id,
                        ..Change::modification()
                    },
                    diff: None,
                };
                match calls {
                    0 => {
                        assert_eq!(src.unwrap(), source_a);
                        assert_eq!(
                            dst.location, "a-cpy-1",
                            "it just finds the first possible match in order, ignoring other candidates"
                        );
                    }
                    1 => {
                        assert_eq!(src.unwrap(), source_a, "copy-sources can be used multiple times");
                        assert_eq!(dst.location, "a-cpy-2");
                    }
                    2 => {
                        assert!(src.is_none());
                        assert_eq!(dst.location, "d");
                    }
                    _ => panic!("too many emissions"),
                }
                calls += 1;
                Action::Continue
            },
            odb,
        );
        assert_eq!(
            out,
            rewrites::Outcome {
                options: rewrites,
                ..Default::default()
            },
            "no similarity check was performed, it was all matched by id"
        );
    }
    Ok(())
}

#[test]
fn copy_by_id_search_in_all_sources() -> crate::Result {
    // Limits are only applied when doing rewrite-checks
    for limit in [0, 1] {
        let rewrites = Rewrites {
            copies: Some(Copies {
                source: CopySource::FromSetOfModifiedFilesAndAllSources,
                percentage: None,
            }),
            percentage: None,
            limit,
        };
        let mut track = util::new_tracker(rewrites);
        let odb = util::add_retained_blobs(
            &mut track,
            [
                (Change::addition(), "a-cpy-1", "a"),
                (Change::addition(), "a-cpy-2", "a"),
            ],
        );

        let mut calls = 0;
        let content_id = hex_to_id("2e65efe2a145dda7ee51d1741299f848e5bf752e");
        let out = util::assert_emit_with_objects_and_sources(
            &mut track,
            |dst, src| {
                let source_a = Source {
                    entry_mode: EntryKind::Blob.into(),
                    id: content_id,
                    kind: SourceKind::Copy,
                    location: "a-src".into(),
                    change: &Change {
                        id: content_id,
                        ..Change::modification()
                    },
                    diff: None,
                };
                match calls {
                    0 => {
                        assert_eq!(src.unwrap(), source_a);
                        assert_eq!(
                            dst.location, "a-cpy-1",
                            "it just finds the first possible match in order, ignoring other candidates"
                        );
                    }
                    1 => {
                        assert_eq!(src.unwrap(), source_a, "copy-sources can be used multiple times");
                        assert_eq!(dst.location, "a-cpy-2");
                    }
                    2 => {
                        assert!(src.is_none());
                        assert_eq!(dst.location, "d");
                    }
                    _ => panic!("too many emissions"),
                }
                calls += 1;
                Action::Continue
            },
            odb,
            [(
                {
                    let mut c = Change::modification();
                    c.id = content_id;
                    c
                },
                "a-src",
            )],
        );
        assert_eq!(
            out,
            rewrites::Outcome {
                options: rewrites,
                ..Default::default()
            },
            "no similarity check was performed, it was all matched by id"
        );
    }
    Ok(())
}

#[test]
fn copy_by_50_percent_similarity() -> crate::Result {
    let rewrites = Rewrites {
        copies: Some(Copies {
            source: CopySource::FromSetOfModifiedFiles,
            percentage: Some(0.5),
        }),
        percentage: None,
        limit: 0,
    };
    let mut track = util::new_tracker(rewrites);
    let odb = util::add_retained_blobs(
        &mut track,
        [
            (Change::modification(), "a", "a\n"),
            (Change::addition(), "a-cpy-1", "a\nb"),
            (Change::addition(), "a-cpy-2", "a\nc"),
            (Change::modification(), "d", "a"),
        ],
    );

    let mut calls = 0;
    let out = util::assert_emit_with_objects(
        &mut track,
        |dst, src| {
            let id = hex_to_id("78981922613b2afb6025042ff6bd878ac1994e85");
            let source_a = Source {
                entry_mode: EntryKind::Blob.into(),
                id: id,
                kind: SourceKind::Copy,
                location: "a".into(),
                change: &Change {
                    id,
                    ..Change::modification()
                },
                diff: Some(DiffLineStats {
                    removals: 0,
                    insertions: 1,
                    before: 1,
                    after: 2,
                    similarity: 0.6666667,
                }),
            };
            match calls {
                0 => {
                    assert_eq!(
                        src.unwrap(),
                        source_a,
                        "it finds the first possible source, no candidates"
                    );
                    assert_eq!(dst.location, "a-cpy-1");
                }
                1 => {
                    assert_eq!(src.unwrap(), source_a, "the same source can be reused as well");
                    assert_eq!(dst.location, "a-cpy-2");
                }
                2 => {
                    assert!(src.is_none());
                    assert_eq!(dst.location, "d");
                }
                _ => panic!("too many emissions"),
            }
            calls += 1;
            Action::Continue
        },
        odb,
    );
    assert_eq!(
        out,
        rewrites::Outcome {
            options: rewrites,
            num_similarity_checks: 4,
            ..Default::default()
        },
        "no similarity check was performed, it was all matched by id"
    );
    Ok(())
}

#[test]
fn copy_by_id_in_additions_only() -> crate::Result {
    let rewrites = Rewrites {
        copies: Some(Copies {
            source: CopySource::FromSetOfModifiedFiles,
            percentage: None,
        }),
        percentage: None,
        limit: 0,
    };
    let mut track = util::new_tracker(rewrites);
    let odb = util::add_retained_blobs(
        &mut track,
        [
            (Change::modification(), "a", "a"),
            (Change::modification(), "a-cpy-1", "a"),
        ],
    );

    let mut calls = 0;
    let out = util::assert_emit_with_objects(
        &mut track,
        |dst, src| {
            match calls {
                0 => {
                    assert!(src.is_none());
                    assert_eq!(dst.location, "a");
                }
                1 => {
                    assert!(src.is_none());
                    assert_eq!(
                        dst.location, "a-cpy-1",
                        "copy detection is only done for additions, not within modifications"
                    );
                }
                _ => panic!("too many emissions"),
            }
            calls += 1;
            Action::Continue
        },
        odb,
    );
    assert_eq!(
        out,
        rewrites::Outcome {
            options: rewrites,
            ..Default::default()
        },
        "no similarity check was performed, it was all matched by id"
    );
    Ok(())
}

#[test]
fn rename_by_similarity_reports_limit_if_encountered() -> crate::Result {
    let rewrites = Rewrites {
        copies: None,
        percentage: Some(0.5),
        limit: 1,
    };
    let mut track = util::new_tracker(rewrites);
    let odb = util::add_retained_blobs(
        &mut track,
        [
            (Change::deletion(), "a", "first\nsecond\n"),
            (Change::addition(), "b", "firt\nsecond\n"),
            (Change::addition(), "c", "second\nunrelated\n"),
        ],
    );

    let mut calls = 0;
    let out = util::assert_emit_with_objects(
        &mut track,
        |dst, src| {
            assert!(src.is_none());
            match calls {
                0 => assert_eq!(dst.location, "a"),
                1 => assert_eq!(dst.location, "b"),
                2 => assert_eq!(dst.location, "c"),
                _ => panic!("too many elements emitted"),
            };
            calls += 1;
            Action::Continue
        },
        odb,
    );
    assert_eq!(
        out,
        rewrites::Outcome {
            options: rewrites,
            num_similarity_checks_skipped_for_rename_tracking_due_to_limit: 2,
            ..Default::default()
        },
        "no similarity check was performed at all - all or nothing"
    );
    Ok(())
}

#[test]
fn rename_by_50_percent_similarity() -> crate::Result {
    let rewrites = Rewrites {
        copies: None,
        percentage: Some(0.5),
        limit: 0,
    };
    let mut track = util::new_tracker(rewrites);
    let odb = util::add_retained_blobs(
        &mut track,
        [
            (Change::deletion(), "a", "first\nsecond\n"),
            (Change::addition(), "b", "firt\nsecond\n"),
            (Change::addition(), "c", "second\nunrelated\n"),
        ],
    );

    let mut calls = 0;
    let out = util::assert_emit_with_objects(
        &mut track,
        |dst, src| {
            match calls {
                0 => {
                    let id = hex_to_id("66a52ee7a1d803dc57859c3e95ac9dcdc87c0164");
                    assert_eq!(
                        src.unwrap(),
                        Source {
                            entry_mode: EntryKind::Blob.into(),
                            id: id,
                            kind: SourceKind::Rename,
                            location: "a".into(),
                            change: &Change {
                                id,
                                ..Change::deletion()
                            },
                            diff: Some(DiffLineStats {
                                removals: 1,
                                insertions: 1,
                                before: 2,
                                after: 2,
                                similarity: 0.53846157
                            })
                        }
                    );
                    assert_eq!(dst.location, "b");
                }
                1 => {
                    assert!(src.is_none(), "pair already found");
                    assert_eq!(dst.location, "c");
                }
                _ => panic!("too many elements emitted"),
            };
            calls += 1;
            Action::Continue
        },
        odb,
    );
    assert_eq!(
        out,
        rewrites::Outcome {
            options: rewrites,
            num_similarity_checks: 1,
            ..Default::default()
        },
        "the first attempt already yields the one pair, so it doesn't participate anymore\
         - we don't have best candidates yet, thus only one check"
    );
    Ok(())
}

#[test]
fn remove_only() -> crate::Result {
    let mut track = util::new_tracker(Default::default());
    assert!(
        track.try_push_change(Change::deletion(), "a".into()).is_none(),
        "recorded for later matching"
    );
    let mut called = false;
    let out = util::assert_emit(&mut track, |dst, src| {
        assert!(!called);
        called = true;
        assert_eq!(src, None, "there is just a single deletion, no pair");
        assert_eq!(dst.location, "a");
        assert_eq!(dst.change.kind, ChangeKind::Deletion);
        Action::Continue
    });
    assert_eq!(out, Default::default());
    Ok(())
}

#[test]
fn add_only() -> crate::Result {
    let mut track = util::new_tracker(Default::default());
    assert!(
        track.try_push_change(Change::addition(), "a".into()).is_none(),
        "recorded for later matching - note that this is the starting point of a matching run"
    );
    let mut called = false;
    let out = util::assert_emit(&mut track, |dst, src| {
        assert!(!called);
        called = true;
        assert!(src.is_none(), "there is just a single deletion, no pair");
        assert_eq!(dst.location, "a");
        assert_eq!(dst.change.kind, ChangeKind::Addition);
        Action::Continue
    });
    assert_eq!(out, Default::default());
    Ok(())
}

mod util {
    use gix_diff::{
        rewrites,
        rewrites::tracker::visit::{Destination, Source},
        tree::visit::Action,
        Rewrites,
    };

    use crate::{rewrites::Change, util::ObjectDb};

    /// Add `blobs` `(change, location, data)` to tracker that will all be retained. Note that the `id` of the respective change will be adjusted to match.
    pub fn add_retained_blobs<'a>(
        tracker: &mut rewrites::Tracker<Change>,
        blobs: impl IntoIterator<Item = (Change, &'a str, &'a str)>,
    ) -> ObjectDb {
        let mut db = ObjectDb::default();
        for (mut change, location, data) in blobs {
            change.id = db.insert(data);
            assert!(
                tracker.try_push_change(change, location.into()).is_none(),
                "input changes must be tracked"
            );
        }
        db
    }

    pub fn assert_emit(
        tracker: &mut rewrites::Tracker<Change>,
        cb: impl FnMut(Destination<'_, Change>, Option<Source<'_, Change>>) -> Action,
    ) -> rewrites::Outcome {
        assert_emit_with_objects(tracker, cb, gix_object::find::Never)
    }

    pub fn assert_emit_with_objects(
        tracker: &mut rewrites::Tracker<Change>,
        cb: impl FnMut(Destination<'_, Change>, Option<Source<'_, Change>>) -> Action,
        objects: impl gix_object::FindObjectOrHeader,
    ) -> rewrites::Outcome {
        assert_emit_with_objects_and_sources(tracker, cb, objects, None)
    }

    pub fn assert_emit_with_objects_and_sources<'a>(
        tracker: &mut rewrites::Tracker<Change>,
        cb: impl FnMut(Destination<'_, Change>, Option<Source<'_, Change>>) -> Action,
        objects: impl gix_object::FindObjectOrHeader,
        sources: impl IntoIterator<Item = (Change, &'a str)>,
    ) -> rewrites::Outcome {
        let mut sources: Vec<_> = sources.into_iter().collect();
        tracker
            .emit(
                cb,
                &mut new_platform_no_worktree(),
                &objects,
                |cb| -> Result<(), std::io::Error> {
                    let sources = std::mem::take(&mut sources);
                    if sources.is_empty() {
                        panic!("Should not access more sources unless these are specified");
                    }
                    for (src, location) in sources {
                        cb(src, location.into());
                    }
                    Ok(())
                },
            )
            .expect("emit doesn't fail")
    }

    pub fn new_tracker(rewrites: Rewrites) -> rewrites::Tracker<Change> {
        rewrites::Tracker::new(rewrites)
    }

    fn new_platform_no_worktree() -> gix_diff::blob::Platform {
        let root = gix_testtools::scripted_fixture_read_only_standalone("make_blob_repo.sh").expect("valid fixture");
        let attributes = gix_worktree::Stack::new(
            root,
            gix_worktree::stack::State::AttributesStack(gix_worktree::stack::state::Attributes::new(
                Default::default(),
                None,
                gix_worktree::stack::state::attributes::Source::IdMapping,
                Default::default(),
            )),
            gix_worktree::glob::pattern::Case::Sensitive,
            Vec::new(),
            Vec::new(),
        );
        let filter = gix_diff::blob::Pipeline::new(
            Default::default(),
            gix_filter::Pipeline::default(),
            Vec::new(),
            Default::default(),
        );
        gix_diff::blob::Platform::new(
            Default::default(),
            filter,
            gix_diff::blob::pipeline::Mode::ToGit,
            attributes,
        )
    }
}
