// Copyright (c) 2004-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

//! Mercurial Types
//!
//! This crate contains useful definitions for types that occur in Mercurial. Or more generally,
//! in a source control system that is based on Mercurial and extensions.
//!
//! The top-most level is the Repo, which is a container for changesets.
//!
//! A changeset represents a snapshot of a file tree at a specific moment in time. Changesets
//! can (and commonly do) have parent-child relationships with other changesets; if once changeset
//! is the child of another one, then it is interpreted as an incremental change in the history of
//! a single namespace. Changesets can have multiple parents (currently limited to 2), which
//! represents the merging of history. A changeset can have no parents, which represents the
//! creation of a new namespace. There's no requirement that all (or any) changeset within a
//! repo be connected at all via parent-child relationships.
//!
//! Each changeset has a tree of manifests, which represent their namespace. A manifest is
//! equivalent to a directory in a filesystem, mapping names to other objects. Those other
//! objects can be other manifests (subdirectories), files, or symlinks. Manifest objects can
//! be shared by multiple changesets - if the only difference between two changesets is a
//! single file, then all other files and directories will be the same and shared.
//!
//! Changesets, manifests and files are uniformly represented by a `Node`. A `Node` has
//! 0-2 parents and some content. A node's identity is computed by hashing over (p1, p2, content),
//! resulting in `NodeHash` (TODO: rename NodeHash -> NodeId?). This means manifests and files
//! have a notion of history independent of the changeset(s) they're embedded in.
//!
//! Nodes are stored as blobs in the blobstore, but with their content in a separate blob. This
//! is because it's very common for the same file content to appear either under different names
//! (copies) or multiple times within the same history (reverts), or both (rebase, amend, etc).
//!
//! Blobs are the underlying raw storage for all immutable objects in Mononoke. Their primary
//! storage key is a hash (TBD, stronger than SHA1) over their raw bit patterns, but they can
//! have other keys to allow direct access via multiple aliases. For example, file content may be
//! shared by multiple nodes, but can be access directly without having to go via a node.
//!
//! Delta and bdiff are used in revlogs and on the wireprotocol to represent inter-file
//! differences. These are for interfacing at the edges, but are not used within Mononoke's core
//! structures at all.
#![deny(warnings)]
#![feature(const_fn)]
#![feature(never_type)]
#![feature(try_from)]

extern crate ascii;
#[cfg(test)]
#[macro_use]
extern crate assert_matches;
extern crate bincode;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate rand;
extern crate rust_crypto;
#[macro_use]
extern crate url;

extern crate futures;

#[macro_use]
extern crate failure;

#[cfg_attr(test, macro_use)]
extern crate quickcheck;

extern crate heapsize;
#[macro_use]
extern crate heapsize_derive;

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate futures_ext;
extern crate storage_types;

pub mod bdiff;
pub mod delta;
pub mod errors;
pub mod hash;
pub mod nodehash;
pub mod path;
pub mod utils;
pub mod manifest;
pub mod blob;
pub mod blobnode;
pub mod changeset;
mod node;

pub use blob::{Blob, BlobHash};
pub use blobnode::{BlobNode, Parents};
pub use changeset::{Changeset, Time};
pub use delta::Delta;
pub use manifest::{Entry, Manifest, Type};
pub use node::Node;
pub use nodehash::{ChangesetId, EntryId, ManifestId, NodeHash, NULL_HASH};
pub use path::{fncache_fsencode, simple_fsencode, MPath, MPathElement, RepoPath};
pub use utils::percent_encode;

pub use errors::{Error, ErrorKind};

#[cfg(test)]
mod test;
