import { describe, expect, it } from "bun:test";
import {
  CrossDomainIdentityError,
  fileContentHash,
  semanticFingerprint,
} from "./domainforge-adapter";

/**
 * CEP-0008 world_ref boundary confirmation (migration Stage 9, boundary 10:
 * "MAY / confirm per spec").
 *
 * Gauntlet keeps its own judgment model and consumes no `world_ref` today.
 * What must hold if a `world_ref` ever rides alongside DomainForge evidence is
 * that it stays a distinct identity: DomainForge defines it
 * (`world:<name>@sha256:<digest>` over `DomainModelIdentity::canonical_digest`),
 * SEA-Forge verifies it, and Gauntlet must never mistake it for one of its own
 * typed identity domains or treat it as a verdict. These tests pin the first
 * half; the second half (no Gauntlet Settlement is created from SXR/DomainForge
 * data) is already pinned by the SXR adapter suites.
 */
const DIGEST = "b".repeat(64);
const WORLD_REF = `world:acme@sha256:${DIGEST}`;

describe("world_ref is not a Gauntlet identity domain", () => {
  it("cannot be minted as a semantic fingerprint", () => {
    expect(() => semanticFingerprint(WORLD_REF)).toThrow(CrossDomainIdentityError);
  });

  it("cannot be minted as a file content identity", () => {
    expect(() => fileContentHash(WORLD_REF)).toThrow(CrossDomainIdentityError);
  });

  it("its bare digest is not a sha256:-prefixed closure hash either", () => {
    // A world digest is a different hash over a different object than a
    // semantic_closure_hash; presenting it unprefixed must not mint one.
    expect(() => semanticFingerprint(DIGEST)).toThrow(CrossDomainIdentityError);
  });

  it("a world alias or label is no more an identity than the ref", () => {
    for (const alias of ["world:acme", "acme", ""]) {
      expect(() => semanticFingerprint(alias)).toThrow(CrossDomainIdentityError);
    }
  });
});
