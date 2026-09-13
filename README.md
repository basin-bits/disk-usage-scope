# duscope

Disks fill up and `du` gives you numbers but not a shape you can scan
quickly. duscope walks a directory tree, sums sizes bottom-up, sorts
children largest-first at every level, and prints either a readable tree or
JSON you can pipe into something else.

It's a small library (`src/lib.rs`) with a thin CLI (`src/main.rs`) on top,
standard library only - no dependencies to audit or update.

## Usage

Build it:

```
cargo build --release
```

Scan the current directory:

```
$ duscope
    12.4 MiB  .
     8.1 MiB    node_modules
     3.2 MiB    target
     1.1 KiB    src
```

Limit how deep the tree prints (the scan itself still walks everything, so
totals stay accurate even for collapsed branches):

```
$ duscope /var/log --depth 1
   842.0 MiB  log
   600.0 MiB    (14 more)
```

Only show the biggest few entries per directory:

```
$ duscope ~/Downloads --top 5
```

Machine-readable output for scripts:

```
$ duscope src --json
{"name":"src","size":1176,"is_dir":true,"children":[{"name":"lib.rs","size":812,"is_dir":false},{"name":"main.rs","size":364,"is_dir":false}]}
```

Every entry has `name`, `size` (bytes), and `is_dir`; directories also carry
`children`. An `error` field shows up on an entry if it couldn't be read
(permission denied, or it vanished mid-scan) - the entry stays in the tree
with size 0 rather than being silently dropped.

## Status

Early. The scanner and both output modes work end to end. Not yet handled:
exclude patterns, a minimum-size filter, and symlink-loop protection beyond
"symlinks aren't followed at all" (see `src/lib.rs`).

## License

MIT, see `LICENSE`.
