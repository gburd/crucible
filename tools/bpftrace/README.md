# Crucible bpftrace scripts (Linux)

The `crucible_downstairs` USDT provider (defined in `downstairs/src/lib.rs`)
exposes the same per-IO probes on Linux that the DTrace scripts in
`../dtrace/` consume on illumos. These `bpftrace` scripts are the Linux
counterparts to the downstairs performance scripts.

## Requirements

- `bpftrace` (which needs root and a kernel with eBPF + uprobe USDT support).
- A `crucible-downstairs` binary built with USDT probes enabled. The probes
  are present in normal builds; confirm with:

  ```
  readelf -n <path-to-crucible-downstairs> | grep -A1 crucible_downstairs
  ```

## Attaching

USDT uprobes are attached per-process, so pass the target PID:

```
sudo bpftrace -p $(pgrep -n crucible-downstairs) tools/bpftrace/perf-downstairs.bt
```

For a `dsc`-spawned fleet there are several downstairs; trace one at a time by
its PID, or run one invocation per PID. Hit Ctrl-C to print the histograms.

## perf-downstairs.bt

Trace each IO from when the downstairs receives it (`submit-*-start`) to when
it has completed and is about to ack the upstairs (`submit-*-done`). Latency
is shown as a power-of-two histogram (nanoseconds) grouped by IO type
(read / write / writeunwritten / flush). This is the Linux equivalent of
`../dtrace/perf-downstairs.d`.

## perf-downstairs-three.bt

Break a downstairs IO into three phases, mirroring
`../dtrace/perf-downstairs-three.d`:

1. `@submit` — IO received (from upstairs) to handing it to the OS.
2. `@os_time` — OS service time (for flush, the time to flush all extents).
3. `@ack` — OS done to the downstairs sending the ACK back to the upstairs.

Each is a per-op-type histogram in nanoseconds.

## Note on probe names

The Rust provider declares probes with `__` separators (e.g.
`submit__read__start`); `usdt` registers them with `-` separators
(`submit-read-start`), which is what both DTrace (`:::submit-read-start`) and
these bpftrace scripts (`usdt:*:crucible_downstairs:submit-read-start`) match.
