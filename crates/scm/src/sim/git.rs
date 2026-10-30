use std::collections::{BTreeMap, BTreeSet};

use super::{Ctx, Repo, Sim};
use crate::{RepoRef, ScmError, ScmUser};

/// The files of a commit: their content by path.
pub(super) type Tree = BTreeMap<String, String>;

#[derive(Debug)]
pub(super) struct Commit {
    parents: Vec<String>,
    tree: Tree,
}

impl Sim {
    /// Creates a commit on `parent` that writes the files with `Some` and
    /// deletes the files with `None`, and returns it.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or parent.
    pub fn commit(
        &self,
        repo: &RepoRef,
        parent: &str,
        files: &[(&str, Option<&str>)],
    ) -> Result<String, ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            let mut tree = ctx.repo.tree(parent)?.clone();
            for (path, content) in files {
                match content {
                    Some(content) => tree.insert((*path).to_owned(), (*content).to_owned()),
                    None => tree.remove(*path),
                };
            }

            Ok(ctx.new_commit(vec![parent.to_owned()], tree))
        })
    }

    /// Moves the branch `branch` to `sha`, as a push of `sender` from
    /// outside CIHub.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown repository or commit.
    pub fn push(
        &self,
        repo: &RepoRef,
        branch: &str,
        sha: &str,
        sender: &ScmUser,
    ) -> Result<(), ScmError> {
        self.with_repo(&repo.external_id, |ctx| {
            ctx.repo.tree(sha)?;
            ctx.move_branch(branch, Some(sha), sender);

            Ok(())
        })
    }
}

impl Ctx<'_> {
    pub(super) fn new_commit(&mut self, parents: Vec<String>, tree: Tree) -> String {
        self.repo.new_commit(self.next_sha, parents, tree)
    }
}

impl Repo {
    pub(super) fn new_commit(
        &mut self,
        next_sha: &mut u64,
        parents: Vec<String>,
        tree: Tree,
    ) -> String {
        *next_sha += 1;
        let sha = format!("{next_sha:040x}");
        self.commits.insert(sha.clone(), Commit { parents, tree });

        sha
    }

    pub(super) fn tree(&self, sha: &str) -> Result<&Tree, ScmError> {
        self.commits
            .get(sha)
            .map(|commit| &commit.tree)
            .ok_or_else(|| ScmError::NotFound(format!("commit {sha}")))
    }

    pub(super) fn branch_head(&self, branch: &str) -> Result<String, ScmError> {
        self.branches
            .get(branch)
            .cloned()
            .ok_or_else(|| ScmError::NotFound(format!("branch {branch}")))
    }

    pub(super) fn is_ancestor(&self, ancestor: &str, sha: &str) -> bool {
        self.ancestors(sha).contains(ancestor)
    }

    pub(super) fn merge_base(&self, a: &str, b: &str) -> Option<String> {
        let common: BTreeSet<String> = self
            .ancestors(a)
            .intersection(&self.ancestors(b))
            .cloned()
            .collect();

        common
            .iter()
            .find(|candidate| {
                common
                    .iter()
                    .all(|other| other == *candidate || !self.is_ancestor(candidate, other))
            })
            .cloned()
    }

    /// The tree of a three-way merge of `theirs` into `ours`, or `None` when
    /// a file changes on both sides.
    pub(super) fn merged_tree(&self, ours: &str, theirs: &str) -> Result<Option<Tree>, ScmError> {
        let empty = Tree::new();
        let base = match self.merge_base(ours, theirs) {
            Some(base) => self.tree(&base)?,
            None => &empty,
        };
        let (ours, theirs) = (self.tree(ours)?, self.tree(theirs)?);
        let paths: BTreeSet<&String> = base
            .keys()
            .chain(ours.keys())
            .chain(theirs.keys())
            .collect();
        let mut merged = Tree::new();
        for path in paths {
            let (o, a, b) = (base.get(path), ours.get(path), theirs.get(path));
            let content = if a == b || b == o {
                a
            } else if a == o {
                b
            } else {
                return Ok(None);
            };
            if let Some(content) = content {
                merged.insert(path.clone(), content.clone());
            }
        }

        Ok(Some(merged))
    }

    fn ancestors(&self, sha: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut next = vec![sha.to_owned()];
        while let Some(sha) = next.pop() {
            if let Some(commit) = self.commits.get(&sha) {
                next.extend(commit.parents.iter().cloned());
            }
            seen.insert(sha);
        }

        seen
    }
}
