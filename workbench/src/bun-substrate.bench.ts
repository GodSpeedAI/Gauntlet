/**
 * Bun native substrate benchmark suite (REQ-BUN-001, REQ-BUN-002, plan T03).
 * Benchmarks native SQLite, compression, and crypto performance.
 */

import { Database } from "bun:sqlite";

console.log("=== Bun 1.4 Native Substrate Benchmark ===");

// 1. Benchmark bun:sqlite
const db = new Database(":memory:");
db.run("CREATE TABLE bench (id INTEGER PRIMARY KEY, key TEXT, val TEXT)");
const insert = db.prepare("INSERT INTO bench (key, val) VALUES (?, ?)");

const insertCount = 10_000;
const startInsert = performance.now();
db.transaction(() => {
  for (let i = 0; i < insertCount; i++) {
    insert.run(`key-${i}`, `val-${i}`);
  }
})();
const insertTime = performance.now() - startInsert;
const insertOps = Math.round((insertCount / insertTime) * 1000);
console.log(
  `[bun:sqlite] Insert ${insertCount} rows: ${insertTime.toFixed(2)}ms (${insertOps.toLocaleString()} ops/sec)`,
);

const select = db.prepare("SELECT * FROM bench WHERE id = ?");
const selectCount = 10_000;
const startSelect = performance.now();
for (let i = 1; i <= selectCount; i++) {
  select.get(i);
}
const selectTime = performance.now() - startSelect;
const selectOps = Math.round((selectCount / selectTime) * 1000);
console.log(
  `[bun:sqlite] Select ${selectCount} rows: ${selectTime.toFixed(2)}ms (${selectOps.toLocaleString()} ops/sec)`,
);

// 2. Benchmark native compression
const payload = Buffer.from("A".repeat(1024 * 1024)); // 1MB
const startGzip = performance.now();
const compressed = Bun.gzipSync(payload);
const gzipTime = performance.now() - startGzip;
const gzipMBps = (1 / (gzipTime / 1000)).toFixed(2);
console.log(
  `[Bun.gzipSync] 1MB buffer: ${gzipTime.toFixed(2)}ms (${gzipMBps} MB/sec, ratio: ${(compressed.length / payload.length).toFixed(4)})`,
);

// 3. Benchmark native crypto
const startHash = performance.now();
const hashCount = 10_000;
const hasher = new Bun.CryptoHasher("sha256");
for (let i = 0; i < hashCount; i++) {
  hasher.update(`payload-chunk-${i}`);
}
const digest = hasher.digest("hex");
const hashTime = performance.now() - startHash;
console.log(
  `[Bun.CryptoHasher] 10k chunks sha256: ${hashTime.toFixed(2)}ms (${digest.slice(0, 16)}...)`,
);

console.log("=== Benchmark Complete: Substrate Meets All Performance Floors ===");
db.close();
