// Copyright (c) 2017-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use futures::executor::spawn;
use quickcheck::{quickcheck, Arbitrary, Gen};
use rand::Rng;
use rand::distributions::Sample;
use rand::distributions::range::Range;
use rand::thread_rng;
use std::collections::HashSet;
use std::sync::Arc;

use blobrepo::BlobRepo;
use mercurial_types::NodeHash;
use repoinfo::RepoGenCache;

use branch_even;
use branch_uneven;
use branch_wide;
use linear;
use merge_even;
use merge_uneven;
use unshared_merge_even;
use unshared_merge_uneven;

use NodeStream;
use intersectnodestream::IntersectNodeStream;
use setdifferencenodestream::SetDifferenceNodeStream;
use singlenodehash::SingleNodeHash;
use unionnodestream::UnionNodeStream;
use validation::ValidateNodeStream;

#[derive(Clone, Copy, Debug)]
enum RevsetEntry {
    SingleNode(Option<NodeHash>),
    SetDifference,
    Intersect(usize),
    Union(usize),
}

#[derive(Clone, Debug)]
pub struct RevsetSpec {
    rp_entries: Vec<RevsetEntry>,
}

impl RevsetSpec {
    pub fn add_hashes<G>(&mut self, repo: &BlobRepo, random: &mut G)
    where
        G: Rng,
    {
        let mut all_changesets_executor = spawn(repo.get_changesets());
        let mut all_changesets: Vec<NodeHash> = Vec::new();
        loop {
            all_changesets.push(match all_changesets_executor.wait_stream() {
                None => break,
                Some(changeset) => changeset.expect("Failed to get changesets from repo"),
            });
        }

        assert!(!all_changesets.is_empty(), "Repo has no changesets");

        for elem in self.rp_entries.iter_mut() {
            if let &mut RevsetEntry::SingleNode(None) = elem {
                *elem = RevsetEntry::SingleNode(random.choose(all_changesets.as_slice()).cloned());
            }
        }
    }

    pub fn as_hashes(&self) -> HashSet<NodeHash> {
        let mut output: Vec<HashSet<NodeHash>> = Vec::new();
        for entry in self.rp_entries.iter() {
            match entry {
                &RevsetEntry::SingleNode(None) => panic!("You need to add_hashes first!"),
                &RevsetEntry::SingleNode(Some(hash)) => {
                    let mut item = HashSet::new();
                    item.insert(hash);
                    output.push(item)
                }
                &RevsetEntry::SetDifference => {
                    let keep = output.pop().expect("No keep for setdifference");
                    let remove = output.pop().expect("No remove for setdifference");
                    output.push(keep.difference(&remove).map(|x| *x).collect())
                }
                &RevsetEntry::Union(size) => {
                    let idx = output.len() - size;
                    let mut inputs = output.split_off(idx).into_iter();
                    let first = inputs.next().expect("No first element");
                    output.push(inputs.fold(first, |a, b| a.union(&b).map(|x| *x).collect()))
                }
                &RevsetEntry::Intersect(size) => {
                    let idx = output.len() - size;
                    let mut inputs = output.split_off(idx).into_iter();
                    let first = inputs.next().expect("No first element");
                    output.push(inputs.fold(first, |a, b| a.intersection(&b).map(|x| *x).collect()))
                }
            }
        }
        assert!(
            output.len() == 1,
            "output should have been length 1, was {}",
            output.len()
        );
        output.pop().expect("No revisions").into_iter().collect()
    }

    pub fn as_revset(&self, repo: Arc<BlobRepo>, repo_generation: RepoGenCache) -> Box<NodeStream> {
        let mut output: Vec<Box<NodeStream>> = Vec::with_capacity(self.rp_entries.len());
        for entry in self.rp_entries.iter() {
            let next_node = ValidateNodeStream::new(
                match entry {
                    &RevsetEntry::SingleNode(None) => panic!("You need to add_hashes first!"),
                    &RevsetEntry::SingleNode(Some(hash)) => {
                        SingleNodeHash::new(hash, &*repo.clone()).boxed()
                    }
                    &RevsetEntry::SetDifference => {
                        let keep = output.pop().expect("No keep for setdifference");
                        let remove = output.pop().expect("No remove for setdifference");
                        SetDifferenceNodeStream::new(
                            &repo.clone(),
                            repo_generation.clone(),
                            keep,
                            remove,
                        ).boxed()
                    }
                    &RevsetEntry::Union(size) => {
                        let idx = output.len() - size;
                        let inputs = output.split_off(idx);
                        UnionNodeStream::new(&repo.clone(), repo_generation.clone(), inputs).boxed()
                    }
                    &RevsetEntry::Intersect(size) => {
                        let idx = output.len() - size;
                        let inputs = output.split_off(idx);
                        IntersectNodeStream::new(&repo.clone(), repo_generation.clone(), inputs)
                            .boxed()
                    }
                },
                &repo.clone(),
                repo_generation.clone(),
            ).boxed();
            output.push(next_node);
        }
        assert!(
            output.len() == 1,
            "output should have been length 1, was {}",
            output.len()
        );
        output.pop().expect("No revset entries")
    }
}

impl Arbitrary for RevsetSpec {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let mut revset: Vec<RevsetEntry> = Vec::with_capacity(g.size());
        let mut revspecs_in_set: usize = 0;
        let mut range = Range::new(0, 4);

        for _ in 0..g.size() {
            if revspecs_in_set == 0 {
                // Can't add a set operator if we have don't have at least one node
                revset.push(RevsetEntry::SingleNode(None));
            } else {
                let input_count = g.gen_range(0, revspecs_in_set) + 1;
                revset.push(
                    // Bias towards SingleNode if we only have 1 rev
                    match range.sample(g) {
                        0 => RevsetEntry::SingleNode(None),
                        1 => if revspecs_in_set >= 2 {
                            revspecs_in_set -= 2;
                            RevsetEntry::SetDifference
                        } else {
                            RevsetEntry::SingleNode(None)
                        },
                        2 => {
                            revspecs_in_set -= input_count;
                            RevsetEntry::Intersect(input_count)
                        }
                        3 => {
                            revspecs_in_set -= input_count;
                            RevsetEntry::Union(input_count)
                        }
                        _ => panic!("Range returned too wide a variation"),
                    },
                );
            }
            revspecs_in_set += 1;
        }
        assert!(revspecs_in_set > 0, "Did not produce enough revs");

        if revspecs_in_set > 1 {
            let mut range = Range::new(0, 2);
            revset.push(match range.sample(g) {
                0 => RevsetEntry::Intersect(revspecs_in_set),
                1 => RevsetEntry::Union(revspecs_in_set),
                _ => panic!("Range returned too wide a variation"),
            });
        }

        RevsetSpec { rp_entries: revset }
    }

    // TODO(simonfar) We should implement shrink(), but we face the issue of ensuring that the
    // resulting revset only contains one final item.
    // Rough sketch: Take the last element of the Vec, so that we're using the same final reduction
    // type. Vector shrink the rest of the Vec using the standard shrinker. Re-add the final
    // reduction type. Note that we then need to handle the case where the final reduction type
    // is a SetDifference by pure chance.
}

fn match_hashset_to_revset(repo: Arc<BlobRepo>, mut set: RevsetSpec) -> bool {
    let repo_generation = RepoGenCache::new(10);

    set.add_hashes(&*repo, &mut thread_rng());
    let mut hashes = set.as_hashes();
    let mut nodestream = spawn(set.as_revset(repo, repo_generation));

    while !hashes.is_empty() {
        let hash = nodestream
            .wait_stream()
            .expect("Unexpected end of stream")
            .expect("Unexpected error");
        if !hashes.remove(&hash) {
            return false;
        }
    }
    nodestream.wait_stream().is_none() && hashes.is_empty()
}

// This is slightly icky. I would like to construct $test_name as setops_$repo, but concat_idents!
// does not work the way I'd like it to. For now, make the user of this macro pass in both idents
macro_rules! quickcheck_setops {
    ($test_name:ident, $repo:ident) => {
        #[test]
        fn $test_name() {
            fn prop(set: RevsetSpec) -> bool {
                let repo = Arc::new($repo::getrepo());
                match_hashset_to_revset(repo, set)
            }
            quickcheck(prop as fn(RevsetSpec) -> bool)
        }
    };
}

quickcheck_setops!(setops_branch_even, branch_even);
quickcheck_setops!(setops_branch_uneven, branch_uneven);
quickcheck_setops!(setops_branch_wide, branch_wide);
quickcheck_setops!(setops_linear, linear);
quickcheck_setops!(setops_merge_even, merge_even);
quickcheck_setops!(setops_merge_uneven, merge_uneven);
quickcheck_setops!(setops_unshared_merge_even, unshared_merge_even);
quickcheck_setops!(setops_unshared_merge_uneven, unshared_merge_uneven);
