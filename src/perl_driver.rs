//! Long-lived Perl-render driver for the regression harness.
//!
//! Spawns one `perl scripts/persistent_driver.pl` subprocess and
//! services many `(date, rubric, hour)` requests over its stdin
//! pipe. Eliminates per-render perl-startup + require-chain +
//! setupstring cold-fill cost — measured ~4× speedup vs
//! `bash scripts/do_render.sh` for serial workloads.
//!
//! See `docs/PERL_INPROC_PLAN.md` (slice F1).
//!
//! Protocol with the driver:
//!
//!   stdin (Rust → driver):
//!     `<DATE>\t<VERSION>\t<HOUR>\n`         — render one
//!     `QUIT\n`                              — graceful shutdown
//!
//!   stdout (driver → Rust):
//!     `LEN <bytes>\n<bytes of UTF-8 HTML>`  — one framed response
//!
//!   stderr (driver → Rust):
//!     `READY\n`                             — handshake (after warmup)
//!     `ERROR ...\n`                         — diagnostic; logged
//!     `WARMUP FAILED: ...\n`                — diagnostic; not fatal
//!
//! Thread model: one driver per thread. `PerlDriver` is `!Sync`
//! (single stdin pipe, owns mutable read state). For a parallel
//! sweep, give each rayon worker its own driver via
//! `thread_local!` or by spawning per-job.

#![cfg(feature = "regression")]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Which DivinumOfficium entrypoint a `PerlDriver` instance owns.
/// Each driver is locked to one type because missa/ordo.pl and
/// horas/horas.pl both define `sub getordinarium` in `package main`
/// — loading both into a single process leaves whichever was
/// loaded last bound to that name globally, breaking the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptType {
    /// SanctaMissa renders via missa/missa.pl.
    Missa,
    /// All Office hours (Matutinum, Laudes, Prima, Tertia, Sexta,
    /// Nona, Vespera, Completorium) render via horas/officium.pl.
    Officium,
}

impl ScriptType {
    /// Map an hour name to its driver type.
    pub fn for_hour(hour: &str) -> Self {
        if hour == "SanctaMissa" {
            Self::Missa
        } else {
            Self::Officium
        }
    }

    fn as_arg(self) -> &'static str {
        match self {
            Self::Missa => "missa",
            Self::Officium => "officium",
        }
    }
}

pub struct PerlDriver {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Type this driver was spawned for. Render requests for the
    /// other type are routed to a separate driver instance.
    pub script_type: ScriptType,
    /// Repo root so we can re-spawn after a crash without
    /// threading the path through every render call.
    #[allow(dead_code)]
    root: PathBuf,
}

impl PerlDriver {
    /// Spawn the driver and block until it emits `READY\n` on
    /// stderr (after the warmup render). The cwd of the perl
    /// process inherits from the parent — paths inside the driver
    /// resolve via the explicit `vendor_root` argv argument, not
    /// cwd, so this is safe under rayon.
    pub fn new(repo_root: &Path, script_type: ScriptType) -> Result<Self, String> {
        let driver_pl = repo_root.join("scripts/persistent_driver.pl");
        if !driver_pl.exists() {
            return Err(format!(
                "scripts/persistent_driver.pl missing at {}",
                driver_pl.display()
            ));
        }
        let vendor = repo_root.join("vendor/divinum-officium");
        if !vendor.join("web/cgi-bin/missa/missa.pl").exists() {
            return Err(format!(
                "vendor/divinum-officium missing at {}",
                vendor.display()
            ));
        }
        let missa_pl = vendor.join("web/cgi-bin/missa/missa.pl");
        let officium_pl = vendor.join("web/cgi-bin/horas/officium.pl");

        let mut child = Command::new("perl")
            .arg(&driver_pl)
            .arg(script_type.as_arg())
            .arg(&vendor)
            .arg(&missa_pl)
            .arg(&officium_pl)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn perl driver: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "perl driver: no stdin handle".to_string())?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| "perl driver: no stdout handle".to_string())?,
        );
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "perl driver: no stderr handle".to_string())?;

        // Block until the driver writes "READY\n" on stderr. We
        // drain everything before "READY" into a buffer for
        // diagnostic surfacing if the warmup failed. Anything
        // written to stderr AFTER ready is forwarded asynchronously
        // by the spawned thread below.
        let mut stderr_reader = BufReader::new(stderr);
        let mut warmup_log = String::new();
        loop {
            let mut line = String::new();
            let n = stderr_reader
                .read_line(&mut line)
                .map_err(|e| format!("read driver stderr: {e}"))?;
            if n == 0 {
                return Err(format!(
                    "perl driver exited before READY:\n{}",
                    warmup_log
                ));
            }
            if line.trim() == "READY" {
                break;
            }
            warmup_log.push_str(&line);
        }

        // Forward post-ready stderr to our own stderr in a worker
        // thread, so error messages from the driver still surface
        // even though we don't actively read it from `render`.
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut reader = stderr_reader.into_inner();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let _ = std::io::stderr().write_all(&buf[..n]);
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin,
            stdout,
            script_type,
            root: repo_root.to_path_buf(),
        })
    }

    /// Render one `(date, rubric, hour)` request. Returns the
    /// rendered HTML body verbatim. Errors are returned as `Err`
    /// — the driver itself stays alive across them, so the caller
    /// can keep going.
    pub fn render(
        &mut self,
        date: &str,
        rubric: &str,
        hour: &str,
    ) -> Result<String, String> {
        // Send the request line. Tab-separated; date / rubric /
        // hour are validated at higher layers and never contain
        // tabs or newlines.
        write!(self.stdin, "{}\t{}\t{}\n", date, rubric, hour)
            .map_err(|e| format!("write driver stdin: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("flush driver stdin: {e}"))?;

        // Read the LEN header.
        let mut header = String::new();
        let n = self
            .stdout
            .read_line(&mut header)
            .map_err(|e| format!("read LEN header: {e}"))?;
        if n == 0 {
            return Err("perl driver closed stdout".into());
        }
        let len: usize = header
            .strip_prefix("LEN ")
            .and_then(|s| s.trim_end().parse().ok())
            .ok_or_else(|| format!("malformed LEN line: {:?}", header))?;

        // Read exactly `len` bytes.
        let mut body = vec![0u8; len];
        self.stdout
            .read_exact(&mut body)
            .map_err(|e| format!("read body ({} bytes): {e}", len))?;

        String::from_utf8(body).map_err(|e| format!("non-utf8 body: {e}"))
    }
}

impl Drop for PerlDriver {
    fn drop(&mut self) {
        // Best-effort graceful shutdown. Ignore errors — the child
        // may already be dead.
        let _ = self.stdin.write_all(b"QUIT\n");
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}
