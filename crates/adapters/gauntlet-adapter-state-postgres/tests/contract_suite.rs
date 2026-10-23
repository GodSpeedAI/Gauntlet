//! PostgreSQL contract suite for EventLog and CapabilityLedger. The shared contract suite (spec §7.3)
//! lives in `gauntlet-ports::contract::event_log` and `gauntlet-ports::contract::capability_ledger`,
//! and this file only supplies the factories the suites parameterize over. The suites run a fresh
//! PostgreSQL namespace per test case, created with a unique name.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use gauntlet_adapter_state_postgres::{
    LOCAL_DEFAULT_URL, PostgresCapabilityLedger, PostgresConfig, PostgresEventLog,
    PostgresRevisionLedger, PostgresStateStore,
};
use gauntlet_ports::capability_ledger::CapabilityLedger;
use gauntlet_ports::contract::capability_ledger::{
    CapabilityLedgerFactory, capability_ledger_suite,
};
use gauntlet_ports::contract::event_log::{EventLogFactory, event_log_suite};
use gauntlet_ports::contract::revision_ledger::{RevisionLedgerFactory, revision_ledger_suite};
use gauntlet_ports::contract::state_store::{StateStoreFactory, state_store_suite};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::{EventLog, EventLogError};
use gauntlet_ports::revision_ledger::RevisionLedger;
use gauntlet_ports::state_store::{RunEvent, StateStore};

/// Factory for creating fresh PostgreSQL namespaces per EventLog test.
/// Each build() creates a new unique namespace with format: gauntlet_t_{pid}_{counter}_{nanos}
struct PostgresEventLogFactory;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

impl EventLogFactory for PostgresEventLogFactory {
    fn label(&self) -> &'static str {
        "postgres-event-log"
    }

    fn build(&self) -> Result<Box<dyn EventLog>, String> {
        let pid = std::process::id();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);

        let url = std::env::var("GAUNTLET_DATABASE_URL")
            .unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

        let config = PostgresConfig { url, namespace };

        PostgresEventLog::open(&config)
            .map(|log| Box::new(log) as Box<dyn EventLog>)
            .map_err(|err| err.to_string())
    }
}

/// Factory for creating fresh PostgreSQL namespaces per CapabilityLedger test.
/// Each build() creates a new unique namespace with format: gauntlet_t_{pid}_{counter}_{nanos}
struct PostgresCapabilityLedgerFactory;

impl CapabilityLedgerFactory for PostgresCapabilityLedgerFactory {
    fn label(&self) -> &'static str {
        "postgres-capability-ledger"
    }

    fn build(&self) -> Result<Box<dyn CapabilityLedger>, String> {
        let pid = std::process::id();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);

        let url = std::env::var("GAUNTLET_DATABASE_URL")
            .unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

        let config = PostgresConfig { url, namespace };

        PostgresCapabilityLedger::open(&config)
            .map(|ledger| Box::new(ledger) as Box<dyn CapabilityLedger>)
            .map_err(|err| err.to_string())
    }
}

/// Factory for creating fresh PostgreSQL namespaces per RevisionLedger test.
/// Each build() creates a new unique namespace with format: gauntlet_t_{pid}_{counter}_{nanos}
struct PostgresRevisionLedgerFactory;

impl RevisionLedgerFactory for PostgresRevisionLedgerFactory {
    fn label(&self) -> &'static str {
        "postgres-revision-ledger"
    }

    fn build(&self) -> Result<Box<dyn RevisionLedger>, String> {
        let pid = std::process::id();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);

        let url = std::env::var("GAUNTLET_DATABASE_URL")
            .unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

        let config = PostgresConfig { url, namespace };

        PostgresRevisionLedger::open(&config)
            .map(|ledger| Box::new(ledger) as Box<dyn RevisionLedger>)
            .map_err(|err| err.to_string())
    }
}

/// Factory for creating fresh PostgreSQL namespaces per StateStore test.
/// Each build() creates a new unique namespace with format: gauntlet_t_{pid}_{counter}_{nanos}
struct PostgresStateStoreFactory;

impl StateStoreFactory for PostgresStateStoreFactory {
    fn label(&self) -> &'static str {
        "postgres-state-store"
    }

    fn build(&self) -> Result<Box<dyn StateStore>, String> {
        let pid = std::process::id();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);

        let url = std::env::var("GAUNTLET_DATABASE_URL")
            .unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

        let config = PostgresConfig { url, namespace };

        PostgresStateStore::open(&config)
            .map(|store| Box::new(store) as Box<dyn StateStore>)
            .map_err(|err| err.to_string())
    }
}

#[test]
fn postgres_event_log_passes_the_shared_contract_suite() {
    let findings = event_log_suite(&PostgresEventLogFactory);
    assert!(
        findings.is_empty(),
        "the event log must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn gapless_offsets_after_a_failed_append_transaction() {
    let pid = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
    let url =
        std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

    let config = PostgresConfig { url, namespace };

    let log = PostgresEventLog::open(&config).expect("log opens");
    let deadline = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_000));

    // Append a valid event to get offset 1 (1-based offsets)
    let event1 = RunEvent {
        run_id: gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
        at: gauntlet_domain::EpochSeconds::new(1_700_000_000),
        kind: gauntlet_ports::state_store::RunEventKind::UnitDispatch {
            unit: gauntlet_domain::UnitId::new("AK-001").unwrap(),
            invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FA0")
                .unwrap(),
        },
    };

    let offset0 = log.append(event1, deadline).expect("first append succeeds");
    assert_eq!(offset0.get(), 1);

    // Try to append a SettlementCommitted event (should fail with SettlementEventNotEarned)
    let settlement_event = RunEvent {
        run_id: gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FBW").unwrap(),
        at: gauntlet_domain::EpochSeconds::new(1_700_000_001),
        kind: gauntlet_ports::state_store::RunEventKind::SettlementCommitted {
            unit_id: gauntlet_domain::UnitId::new("AK-002").unwrap(),
            evidence: gauntlet_domain::EvidenceRef::new("sha256:test").unwrap(),
            verification: gauntlet_domain::VerificationRef::new("ver-123").unwrap(),
            reference_digest: gauntlet_domain::Digest::new("sha256", "aabbccddee").unwrap(),
        },
    };

    let result = log.append(settlement_event, deadline);
    assert!(
        matches!(result, Err(EventLogError::SettlementEventNotEarned { .. })),
        "settlement event should be rejected: {:?}",
        result
    );

    // Append another valid event and verify the offset is contiguous (should be 2, not skipped)
    let event2 = RunEvent {
        run_id: gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FBX").unwrap(),
        at: gauntlet_domain::EpochSeconds::new(1_700_000_002),
        kind: gauntlet_ports::state_store::RunEventKind::UnitDispatch {
            unit: gauntlet_domain::UnitId::new("AK-003").unwrap(),
            invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FA1")
                .unwrap(),
        },
    };

    let offset1 = log
        .append(event2, deadline)
        .expect("second append succeeds");
    assert_eq!(
        offset1.get(),
        2,
        "offset should be contiguous after a failed transaction (1-based offsets)"
    );
}

#[test]
fn concurrent_appends_yield_distinct_contiguous_offsets() {
    let pid = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
    let url =
        std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

    let config = PostgresConfig { url, namespace };

    let log = std::sync::Arc::new(PostgresEventLog::open(&config).expect("log opens"));
    let deadline = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_000));

    let mut handles = vec![];

    // Spawn two threads that both append events
    for thread_id in 0..2 {
        let log_clone = log.clone();
        let handle = std::thread::spawn(move || {
            let mut offsets = vec![];
            for i in 0..5 {
                let run_id = format!("0{:X}ARZ3NDEKTSV4RRFFQ69G5FA{}", thread_id, i);
                let event = RunEvent {
                    run_id: gauntlet_domain::RunId::new(&run_id).unwrap_or_else(|_| {
                        gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
                    }),
                    at: gauntlet_domain::EpochSeconds::new(1_700_000_000 + i as u64),
                    kind: gauntlet_ports::state_store::RunEventKind::UnitDispatch {
                        unit: gauntlet_domain::UnitId::new(&format!(
                            "AK-{:03}",
                            thread_id * 10 + i
                        ))
                        .unwrap(),
                        invocation_id: gauntlet_domain::InvocationId::new(&format!(
                            "0{:X}ARZ3NDEKTSV4RRFFQ69G5FA{}",
                            thread_id, i
                        ))
                        .unwrap_or_else(|_| {
                            gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FA0")
                                .unwrap()
                        }),
                    },
                };
                let offset = log_clone.append(event, deadline).expect("append succeeds");
                offsets.push(offset.get());
            }
            offsets
        });
        handles.push(handle);
    }

    // Collect all offsets from both threads
    let mut all_offsets = vec![];
    for handle in handles {
        let offsets = handle.join().expect("thread completes");
        all_offsets.extend(offsets);
    }

    // Verify that all offsets are unique and contiguous (1-based: 1,2,3,...,10)
    all_offsets.sort();
    for (i, &offset) in all_offsets.iter().enumerate() {
        let expected = (i + 1) as u64; // 1-based offsets
        assert_eq!(
            offset, expected,
            "offsets should be contiguous (1-based): expected {}, got {}",
            expected, offset
        );
    }

    // Also verify by reading from the log
    let last = log.last_offset(deadline).expect("last_offset succeeds");
    assert_eq!(
        last,
        Some(gauntlet_domain::EventLogOffset::new(10)),
        "last offset should be 10 after 10 total appends (1-based offsets)"
    );
}

#[test]
fn first_append_returns_offset_1() {
    let pid = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
    let url =
        std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

    let config = PostgresConfig { url, namespace };
    let log = PostgresEventLog::open(&config).expect("log opens");
    let deadline = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_000));

    // First append should return offset 1 (1-based, not 0-based)
    let event = RunEvent {
        run_id: gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
        at: gauntlet_domain::EpochSeconds::new(1_700_000_000),
        kind: gauntlet_ports::state_store::RunEventKind::UnitDispatch {
            unit: gauntlet_domain::UnitId::new("AK-001").unwrap(),
            invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FA0")
                .unwrap(),
        },
    };

    let offset = log.append(event, deadline).expect("first append succeeds");
    assert_eq!(
        offset.get(),
        1,
        "first offset should be 1 (1-based, matching SQLite AUTOINCREMENT)"
    );
}

#[test]
fn last_offset_none_on_empty_log() {
    let pid = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
    let url =
        std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

    let config = PostgresConfig { url, namespace };
    let log = PostgresEventLog::open(&config).expect("log opens");
    let deadline = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_000));

    // Empty log should return None
    let last = log.last_offset(deadline).expect("last_offset succeeds");
    assert_eq!(last, None, "empty log should return None for last_offset");
}

#[test]
fn decode_error_surfaces_as_damaged_not_empty() {
    let pid = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
    let url =
        std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

    let config = PostgresConfig {
        url: url.clone(),
        namespace: namespace.clone(),
    };
    let log = PostgresEventLog::open(&config).expect("log opens");
    let deadline = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_000));

    // First, append a valid event
    let event = RunEvent {
        run_id: gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
        at: gauntlet_domain::EpochSeconds::new(1_700_000_000),
        kind: gauntlet_ports::state_store::RunEventKind::UnitDispatch {
            unit: gauntlet_domain::UnitId::new("AK-001").unwrap(),
            invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FA0")
                .unwrap(),
        },
    };
    let _ = log.append(event, deadline).expect("append succeeds");

    // Now corrupt the payload by inserting invalid data via raw client
    let mut raw_client = postgres::Client::connect(&url, postgres::NoTls).expect("connect");
    raw_client
        .execute(&format!("SET search_path TO {}", namespace), &[])
        .expect("set search_path");
    raw_client
        .execute(
            "INSERT INTO event_log (\"offset\", run_id, kind, payload, at) VALUES ($1, $2, $3, $4, $5)",
            &[&2i64, &"01ARZ3NDEKTSV4RRFFQ69G5FAV", &"unit_dispatch", &"INVALID_JSON_PAYLOAD", &1700000001i64],
        )
        .expect("insert corrupt row");

    // Try to read from offset 1 - should detect damage, not return empty
    let result = log.read_from(gauntlet_domain::EventLogOffset::new(1), deadline);
    match result {
        Err(EventLogError::Damaged { .. }) => {} // Expected
        Ok(entries) if entries.is_empty() => {
            panic!("decode error was silently swallowed, returning empty log")
        }
        Ok(_) => panic!("decode error was not detected"),
        Err(e) => panic!("wrong error type: {:?}", e),
    }
}

#[test]
fn deleted_middle_row_is_damaged() {
    let pid = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let namespace = format!("gauntlet_t_{}_{:x}_{:x}", pid, counter, nanos);
    let url =
        std::env::var("GAUNTLET_DATABASE_URL").unwrap_or_else(|_| LOCAL_DEFAULT_URL.to_string());

    let config = PostgresConfig {
        url: url.clone(),
        namespace: namespace.clone(),
    };
    let log = PostgresEventLog::open(&config).expect("log opens");
    let deadline = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(1_700_000_000));

    // Append 3 events
    for i in 0..3 {
        let event = RunEvent {
            run_id: gauntlet_domain::RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
            at: gauntlet_domain::EpochSeconds::new(1_700_000_000 + i),
            kind: gauntlet_ports::state_store::RunEventKind::UnitDispatch {
                unit: gauntlet_domain::UnitId::new(&format!("AK-{:03}", i)).unwrap(),
                invocation_id: gauntlet_domain::InvocationId::new(&format!(
                    "01ARZ3NDEKTSV4RRFFQ69G5FA{}",
                    i
                ))
                .unwrap(),
            },
        };
        let _ = log.append(event, deadline).expect("append succeeds");
    }

    // Delete the middle row via raw client
    let mut raw_client = postgres::Client::connect(&url, postgres::NoTls).expect("connect");
    raw_client
        .execute(&format!("SET search_path TO {}", namespace), &[])
        .expect("set search_path");
    raw_client
        .execute("DELETE FROM event_log WHERE \"offset\" = 2", &[])
        .expect("delete middle row");

    // Try to read - should detect the hole and report Damaged
    let result = log.read_from(gauntlet_domain::EventLogOffset::new(1), deadline);
    match result {
        Err(EventLogError::Damaged { detail }) => {
            assert!(
                detail.contains("expected") && detail.contains("found"),
                "should mention the hole: {}",
                detail
            );
        }
        Ok(entries) => panic!("hole was not detected, got {} entries", entries.len()),
        Err(e) => panic!("wrong error type: {:?}", e),
    }
}

#[test]
fn postgres_capability_ledger_passes_the_shared_contract_suite() {
    let findings = capability_ledger_suite(&PostgresCapabilityLedgerFactory);
    assert!(
        findings.is_empty(),
        "the capability ledger must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn postgres_revision_ledger_passes_the_shared_contract_suite() {
    let findings = revision_ledger_suite(&PostgresRevisionLedgerFactory);
    assert!(
        findings.is_empty(),
        "the revision ledger must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn postgres_state_store_passes_the_shared_contract_suite() {
    let findings = state_store_suite(&PostgresStateStoreFactory);
    assert!(
        findings.is_empty(),
        "the state store must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
