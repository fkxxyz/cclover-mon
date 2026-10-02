use super::*;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FILE_BYTES: usize = 256 * 1024;
const WRITE_BYTES: usize = 16 * 1024;
const READ_BYTES: usize = 32 * 1024;
const PARTIAL_BYTES: usize = 4 * 1024;
const PARTIAL_REQUEST_BYTES: usize = 16 * 1024;
const CACHE_HIT_BYTES: usize = 64 * 1024;
const EVENT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct IoTotals {
    read: u64,
    write: u64,
}

enum WorkloadCommand {
    Write { offset: u64, bytes: usize },
    Read { offset: u64, request_bytes: usize },
    FailedRead,
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkloadResult {
    Completed(usize),
    Failed,
}

pub(super) fn run_native_semantic_validation() -> Result<(), String> {
    let directory = unique_test_directory();
    fs::create_dir(&directory).map_err(|error| format!("create test directory: {error}"))?;
    let result = run_in_directory(&directory);
    let cleanup = fs::remove_dir_all(&directory);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(format!("remove test directory: {error}")),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn run_in_directory(directory: &std::path::Path) -> Result<(), String> {
    let path = directory.join("workload.bin");
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("create workload file: {error}"))?;
    file.write_all(&vec![0x5a; FILE_BYTES])
        .map_err(|error| format!("seed workload file: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("flush workload seed: {error}"))?;

    // Warm this region before ETW starts. Reading it again later exercises buffered logical
    // FileIo after the data is already resident in the system cache.
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("seek for cache warmup: {error}"))?;
    let mut warm = vec![0_u8; CACHE_HIT_BYTES];
    file.read_exact(&mut warm)
        .map_err(|error| format!("warm workload cache: {error}"))?;

    // Keep both handles open before the seed session starts. The first post-start write therefore
    // depends on production FileIo rundown seeding rather than a post-start create/name event.
    let write_only = OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(|error| format!("open write-only workload handle: {error}"))?;

    let mut collector = Collector::new();
    let mut totals = collect_current_process_totals(&mut collector)?;

    let (command_tx, command_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = thread::spawn(move || workload_worker(file, write_only, command_rx, result_tx));

    totals = run_case(
        "exact buffered write",
        &mut collector,
        &command_tx,
        &result_rx,
        totals,
        WorkloadCommand::Write {
            offset: 64 * 1024,
            bytes: WRITE_BYTES,
        },
        WorkloadResult::Completed(WRITE_BYTES),
        IoTotals {
            read: 0,
            write: WRITE_BYTES as u64,
        },
    )?;

    totals = run_case(
        "exact buffered read",
        &mut collector,
        &command_tx,
        &result_rx,
        totals,
        WorkloadCommand::Read {
            offset: 128 * 1024,
            request_bytes: READ_BYTES,
        },
        WorkloadResult::Completed(READ_BYTES),
        IoTotals {
            read: READ_BYTES as u64,
            write: 0,
        },
    )?;

    totals = run_case(
        "partial read at EOF",
        &mut collector,
        &command_tx,
        &result_rx,
        totals,
        WorkloadCommand::Read {
            offset: (FILE_BYTES - PARTIAL_BYTES) as u64,
            request_bytes: PARTIAL_REQUEST_BYTES,
        },
        WorkloadResult::Completed(PARTIAL_BYTES),
        IoTotals {
            read: PARTIAL_BYTES as u64,
            write: 0,
        },
    )?;

    totals = run_case(
        "zero-byte EOF read",
        &mut collector,
        &command_tx,
        &result_rx,
        totals,
        WorkloadCommand::Read {
            offset: FILE_BYTES as u64,
            request_bytes: READ_BYTES,
        },
        WorkloadResult::Completed(0),
        IoTotals::default(),
    )?;

    totals = run_case(
        "failed read",
        &mut collector,
        &command_tx,
        &result_rx,
        totals,
        WorkloadCommand::FailedRead,
        WorkloadResult::Failed,
        IoTotals::default(),
    )?;

    let _ = run_case(
        "cache-hit buffered read",
        &mut collector,
        &command_tx,
        &result_rx,
        totals,
        WorkloadCommand::Read {
            offset: 0,
            request_bytes: CACHE_HIT_BYTES,
        },
        WorkloadResult::Completed(CACHE_HIT_BYTES),
        IoTotals {
            read: CACHE_HIT_BYTES as u64,
            write: 0,
        },
    )?;

    command_tx
        .send(WorkloadCommand::Stop)
        .map_err(|error| format!("stop workload worker: {error}"))?;
    worker
        .join()
        .map_err(|_| "workload worker panicked".to_owned())?;
    drop(collector);
    Ok(())
}

fn workload_worker(
    mut file: File,
    mut write_only: File,
    commands: Receiver<WorkloadCommand>,
    results: Sender<Result<WorkloadResult, String>>,
) {
    while let Ok(command) = commands.recv() {
        let result = match command {
            WorkloadCommand::Write { offset, bytes } => file
                .seek(SeekFrom::Start(offset))
                .and_then(|_| file.write_all(&vec![0xa5; bytes]))
                .map(|_| WorkloadResult::Completed(bytes))
                .map_err(|error| format!("perform workload write: {error}")),
            WorkloadCommand::Read {
                offset,
                request_bytes,
            } => file
                .seek(SeekFrom::Start(offset))
                .and_then(|_| {
                    let mut bytes = vec![0_u8; request_bytes];
                    file.read(&mut bytes)
                })
                .map(WorkloadResult::Completed)
                .map_err(|error| format!("perform workload read: {error}")),
            WorkloadCommand::FailedRead => {
                let mut byte = [0_u8; 1];
                match write_only.read(&mut byte) {
                    Ok(bytes) => Err(format!(
                        "read through write-only handle unexpectedly completed {bytes} bytes"
                    )),
                    Err(_) => Ok(WorkloadResult::Failed),
                }
            }
            WorkloadCommand::Stop => break,
        };
        if results.send(result).is_err() {
            break;
        }
    }
}

fn run_case(
    name: &str,
    collector: &mut Collector,
    commands: &Sender<WorkloadCommand>,
    results: &Receiver<Result<WorkloadResult, String>>,
    before: IoTotals,
    command: WorkloadCommand,
    expected_result: WorkloadResult,
    expected_delta: IoTotals,
) -> Result<IoTotals, String> {
    commands
        .send(command)
        .map_err(|error| format!("{name}: send workload command: {error}"))?;
    let result = results
        .recv()
        .map_err(|error| format!("{name}: receive workload result: {error}"))??;
    if result != expected_result {
        return Err(format!(
            "{name}: workload result {result:?}, expected {expected_result:?}"
        ));
    }

    let deadline = Instant::now() + EVENT_DELIVERY_TIMEOUT;
    loop {
        thread::sleep(Duration::from_millis(25));
        let after = collect_current_process_totals(collector)?;
        let delta = delta(after, before);
        if expected_delta != IoTotals::default() && delta == expected_delta {
            return Ok(after);
        }
        if delta.read > expected_delta.read || delta.write > expected_delta.write {
            return Err(format!(
                "{name}: attributed delta {delta:?} exceeded expected {expected_delta:?}"
            ));
        }
        if Instant::now() >= deadline {
            if delta == expected_delta {
                return Ok(after);
            }
            return Err(format!(
                "{name}: attributed delta {delta:?}, expected {expected_delta:?}"
            ));
        }
    }
}

fn collect_current_process_totals(collector: &mut Collector) -> Result<IoTotals, String> {
    let processes = crate::windows::process::collect(None);
    let disks = crate::windows::disk::collect_batch(None);
    let mut notes = Vec::new();
    let rows = collector.collect(&processes, &disks, Some(&mut notes));
    let Some(rows) = rows.value() else {
        return Err(format!(
            "ETW disk attribution unavailable ({:?}): {notes:?}",
            rows.status()
        ));
    };
    let pid = std::process::id();
    Ok(rows.iter().filter(|row| row.process.pid == pid).fold(
        IoTotals::default(),
        |mut total, row| {
            total.read = total.read.saturating_add(row.read_bytes);
            total.write = total.write.saturating_add(row.write_bytes);
            total
        },
    ))
}

fn delta(after: IoTotals, before: IoTotals) -> IoTotals {
    IoTotals {
        read: after.read.saturating_sub(before.read),
        write: after.write.saturating_sub(before.write),
    }
}

fn unique_test_directory() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "cclover-etw-semantics-{}-{nonce}",
        std::process::id()
    ))
}
