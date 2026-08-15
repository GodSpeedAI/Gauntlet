//! Evidence adapter (spec §4.1, §18.2; plan Task 6): a content-addressed,
//! write-once `EvidenceStore` on the filesystem, with per-run pinning and no
//! overwrite path in the API at all.
