# The storage probe

How the hardlink capability of the operator's data root is established, and why
the check runs against a real disk while staying fully testable.

The *what* and *why* — the filesystem contract, and that its violation must be
made visible — are in the spec's
[C5 storage feature](https://github.com/lemonfiber/spec/blob/main/10-functional/features/c-trust/c5-storage.md).
This is the Rust mechanics.

## The one rule the probe defends

Every service that touches the library gets one data mount, with downloads and
media as subdirectories beneath it on one filesystem, so an import *hardlinks*
instead of copying: instant, free, and leaving the file seedable. When that
breaks nothing announces it — imports still succeed and the library still fills,
and the only symptoms are a disk at twice the size it should be and torrents that
cannot seed. The probe turns that invisible property into a checked one.

## Tested, never inferred

Capability is proven by making a hardlink, not read off the filesystem type:

1. Resolve the data root's symlinks first, so the test runs against the real
   volume rather than one a link points away from.
2. Create a file under it, hardlink that file, and read back both names' inode
   and link count.
3. Same inode and a raised count is a hardlink; anything else is not, whatever
   the call returned. Clean up both names regardless.

A filesystem type is a hint; a successful link is a fact. The exceptions are
common enough that inference would be wrong in exactly the cases that matter —
exFAT cannot hardlink at all, SMB on macOS will not expose links usably, and the
Windows side of the WSL2 boundary breaks them.

## Why there is no filesystem port

The seam everything else in the crate sits behind is deliberately *absent* here,
for the reason set out in [ports-and-adapters.md](ports-and-adapters.md): a faked
filesystem would happily report links working on a volume where they do not,
which is the one silent degradation this exists to catch. So the probe touches
the real disk, the same way the config store and the stack materialiser do.

## Keeping it testable anyway

The impure step is kept to observing — resolve, read the mount table, try to
link — and hands back plain values. Every judgement then lives in a pure reading
of those values:

- **`interpret`** turns an observation into the one finding it amounts to.
- **`classify_medium`** is a pure function of the path, the mount-table text, and
  the host, so every platform's reading is reachable from any one machine — the
  same shape as [`platform.rs`](../../crates/lemonfiber-core/src/platform.rs),
  where only a single constant is chosen by the build target.
- **`limitation`** names the specific cause — exFAT, SMB on macOS, the WSL2
  boundary — from the filesystem type first and the environment where the type
  alone does not settle it.

This is what lets the coverage gate hold at 100%. The consequences a real disk
will not reproduce on demand — a network export, an exFAT stick, the far side of
WSL2 — are all reached by handing the pure reading the values they would produce.
The disk itself is only ever asked to do the one thing a fake could lie about.

## Consequences, not properties

"Hardlinks unsupported" means nothing to most operators, so a failed probe is
stated as what they will live with: imports copy instead of link, each taking
minutes rather than being instant, using twice the disk while it runs, and
leaving torrents unable to seed from the library copy. Then the specific cause
and its remedy — a different location where the type can never link, NFS where
SMB will not, the Linux side of the WSL2 boundary.

## What is not here yet

The probe settles capability and mode. The rest of C5 — projected space
exhaustion, availability monitoring while services run, and the operator- versus
service-facing permission split — arrives with the checks that own those
questions. The mount-table reading is Linux-only today; elsewhere the medium is
unknown and the environment carries the naming, which is enough for the platforms
where the boundary cases actually live.

## Related

- [module-layout.md](module-layout.md) · [ports-and-adapters.md](ports-and-adapters.md)
- [error-model.md](error-model.md) — the finding and remedy shape every check uses
