#![allow(missing_docs)]
use std::ops::DerefMut;

use git_hash::ObjectId;
use git_odb::Find;

use crate::{
    easy,
    easy::{Oid, Reference},
};

pub(crate) enum Backing {
    OwnedPacked {
        /// The validated full name of the reference.
        name: git_ref::FullName,
        /// The target object id of the reference, hex encoded.
        target: ObjectId,
        /// The fully peeled object id, hex encoded, that the ref is ultimately pointing to
        /// i.e. when all indirections are removed.
        object: Option<ObjectId>,
    },
    LooseFile(git_ref::file::loose::Reference),
}

pub mod edit {
    use crate::easy;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error(transparent)]
        FileTransactionPrepare(#[from] git_ref::file::transaction::prepare::Error),
        #[error(transparent)]
        FileTransactionCommit(#[from] git_ref::file::transaction::commit::Error),
        #[error(transparent)]
        NameValidation(#[from] git_validate::reference::name::Error),
        #[error("BUG: The repository could not be borrowed")]
        BorrowRepo(#[from] easy::borrow::repo::Error),
    }
}

pub mod peel_to_oid_in_place {
    use crate::easy;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error(transparent)]
        LoosePeelToId(#[from] git_ref::file::loose::reference::peel::to_id::Error),
        #[error(transparent)]
        PackedRefsOpen(#[from] git_ref::packed::buffer::open::Error),
        #[error("BUG: Part of interior state could not be borrowed.")]
        BorrowState(#[from] easy::borrow::state::Error),
        #[error("BUG: The repository could not be borrowed")]
        BorrowRepo(#[from] easy::borrow::repo::Error),
    }
}

// TODO: think about how to detach a Reference. It should essentially be a 'Raw' reference that should exist in `git-ref` rather than here.
impl<'repo, A> Reference<'repo, A>
where
    A: easy::Access + Sized,
{
    pub(crate) fn from_file_ref(reference: git_ref::file::Reference<'_>, access: &'repo A) -> Self {
        Reference {
            backing: match reference {
                git_ref::file::Reference::Packed(p) => Backing::OwnedPacked {
                    name: p.name.into(),
                    target: p.target(),
                    object: p
                        .object
                        .map(|hex| ObjectId::from_hex(hex).expect("a hash kind we know")),
                },
                git_ref::file::Reference::Loose(l) => Backing::LooseFile(l),
            }
            .into(),
            access,
        }
    }
    pub fn target(&self) -> git_ref::Target {
        match self.backing.as_ref().expect("always set") {
            Backing::OwnedPacked { target, .. } => git_ref::Target::Peeled(target.to_owned()),
            Backing::LooseFile(r) => r.target.clone(),
        }
    }

    pub fn name(&self) -> git_ref::FullNameRef<'_> {
        match self.backing.as_ref().expect("always set") {
            Backing::OwnedPacked { name, .. } => name,
            Backing::LooseFile(r) => &r.name,
        }
        .to_ref()
    }

    pub fn peel_to_oid_in_place(&mut self) -> Result<Oid<'repo, A>, peel_to_oid_in_place::Error> {
        let repo = self.access.repo()?;
        match self.backing.take().expect("a ref must be set") {
            Backing::LooseFile(mut r) => {
                let state = self.access.state();
                let mut pack_cache = state.try_borrow_mut_pack_cache()?;
                let oid = r
                    .peel_to_id_in_place(
                        &repo.refs,
                        state.assure_packed_refs_uptodate(&repo.refs)?.as_ref(),
                        |oid, buf| {
                            repo.odb
                                .try_find(oid, buf, pack_cache.deref_mut())
                                .map(|po| po.map(|o| (o.kind, o.data)))
                        },
                    )?
                    .to_owned();
                self.backing = Backing::LooseFile(r).into();
                Ok(Oid::from_id(oid, self.access))
            }
            Backing::OwnedPacked {
                mut target,
                mut object,
                name,
            } => {
                if let Some(peeled_id) = object.take() {
                    target = peeled_id;
                }
                self.backing = Backing::OwnedPacked {
                    name,
                    target,
                    object: None,
                }
                .into();
                Ok(Oid::from_id(target, self.access))
            }
        }
    }
}

pub mod find {
    use crate::easy;

    pub mod existing {

        use crate::easy::reference::find;

        #[derive(Debug, thiserror::Error)]
        pub enum Error {
            #[error(transparent)]
            Find(#[from] find::Error),
            #[error("The reference did not exist even though that was expected")]
            NotFound,
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error(transparent)]
        Find(#[from] git_ref::file::find::Error),
        #[error(transparent)]
        PackedRefsOpen(#[from] git_ref::packed::buffer::open::Error),
        #[error("BUG: Part of interior state could not be borrowed.")]
        BorrowState(#[from] easy::borrow::state::Error),
        #[error("BUG: The repository could not be borrowed")]
        BorrowRepo(#[from] easy::borrow::repo::Error),
    }
}
