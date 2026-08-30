//! The reference pipeline's application services (plan Task 7): the
//! normalization and digest that reproduce `scripts/reference-digest.sh`
//! byte for byte ([`normalize`]), the strict answer-key parser with
//! per-block error isolation ([`parse`]), the strict manifest reader
//! ([`manifest`]), and the [`provider::FileReferenceProvider`] that binds
//! them into the §6.2 port pipeline: resolve → normalize → digest → freeze
//! → `verify_unchanged`.
//!
//! Everything here treats the answer key and the manifest as untrusted
//! external input (spec §21.2): parsed strictly, validated against the
//! domain constructors, and only then admitted. The frozen ruler
//! (`.wayfinder/`, `.gauntlet/reference.lock`) is never written by anything
//! in this module.

pub mod manifest;
pub mod normalize;
pub mod parse;
pub mod provider;
pub mod validate;
