/**
 * Prime Agent recursive session topology projection and reconstruction.
 * Conforms to spec §20.2, REQ-PRIME-002, and plan T04.
 *
 * Reconstructs exact parent-child hierarchy from durable session records without
 * scraping Prime terminal UI screens or generating synthetic replacement IDs (Attack 2).
 */

import type { PrimeMailboxMessage } from "./prime-host";

export interface PrimeSessionRecord {
  readonly sessionId: string;
  readonly parentId: string | null;
  readonly model: string;
  readonly status: "admitted" | "running" | "completed" | "failed";
  readonly contractPath: string;
  readonly messages: readonly PrimeMailboxMessage[];
  readonly createdAt: string;
}

export interface PrimeTopologyNode {
  readonly sessionId: string;
  readonly parentId: string | null;
  readonly model: string;
  readonly status: string;
  readonly contractPath: string;
  readonly children: PrimeTopologyNode[];
  readonly messages: readonly PrimeMailboxMessage[];
  readonly createdAt: string;
}

/**
 * Reconstruct the recursive parent-child topology from durable session records.
 * Preserves exact session IDs and lineage without synthetic replacements (REQ-PRIME-002, Attack 2).
 */
export function reconstructTopology(records: readonly PrimeSessionRecord[]): PrimeTopologyNode[] {
  const nodeMap = new Map<string, PrimeTopologyNode>();
  const rootNodes: PrimeTopologyNode[] = [];

  // 1. Create a node for every durable record preserving exact identity
  for (const record of records) {
    nodeMap.set(record.sessionId, {
      sessionId: record.sessionId,
      parentId: record.parentId,
      model: record.model,
      status: record.status,
      contractPath: record.contractPath,
      children: [],
      messages: [...record.messages],
      createdAt: record.createdAt,
    });
  }

  // 2. Wire parent-child edges based on actual recorded parentId
  for (const record of records) {
    const node = nodeMap.get(record.sessionId);
    if (!node) continue;

    if (record.parentId && nodeMap.has(record.parentId)) {
      const parentNode = nodeMap.get(record.parentId);
      parentNode?.children.push(node);
    } else {
      // No parent or parent outside set -> top-level root in this scope
      rootNodes.push(node);
    }
  }

  return rootNodes;
}

/**
 * Find a specific session node anywhere in the recursive topology tree.
 */
export function findSessionInTopology(
  roots: readonly PrimeTopologyNode[],
  sessionId: string,
): PrimeTopologyNode | null {
  for (const root of roots) {
    if (root.sessionId === sessionId) {
      return root;
    }
    const found = findSessionInTopology(root.children, sessionId);
    if (found) {
      return found;
    }
  }
  return null;
}
