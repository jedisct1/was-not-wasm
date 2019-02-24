# WAS (not WASM)

An overparanoid memory allocator to make WebAssembly applications more
predictable.

## Blurb

The WebAssembly memory model doesn't offer any protection against buffer
underflows/overflows.

As long as accesses are made within the bounds of the linear memory
segments, no page faults will ever occur.

Besides facilitating heartbleed-class vulnerabilities, this memory
model is also painful for application developers.

Where a native application may crash in the same context,
out-of-bounds accesses in a WebAssembly application may cause silent
memory corruption and subtle, tedious-to-debug bugs.

WAS (not WASM) is a simple memory allocator designed to catch memory
issues in WebAssembly compilers and applications.

WAS (not WASM) makes the heap inaccessible, except for pages
explicitly allocated by the application.

WAS (not WASM) makes static data read-only. Writing to a `NULL`
pointer will fault.

WAS (not WASM) never reuses allocated pages after they are `free()`d.
Deallocated pages become inaccessible.

WAS (not WASM) ensures that a guard page immediately follows every
single allocation, so that a single-byte overflow will cause a fault.

WAS (not WASM) inserts a canary before partially allocated pages, and
verifies that it hasn't been tampered in order to detected underflows.

WAS (not WASM) keeps track of the number of allocations, deallocations
and total memory usage, so you can scream at how much of these
WebAssembly applications do, and optimize yours accordingly.

WAS (not WASM) is not designed to be fast. It is designed to help you
develop safer applications. Or faster applications, by using unsafe
constructions with more confidence.

## Installation

Install Rust, and use `cargo`:

```sh
cargo install
```

## Usage

```
USAGE:
    was [FLAGS] [OPTIONS] --file <file>

FLAGS:
    -c, --canary-check-on-alloc
    -h, --help                     Prints help information
    -V, --version                  Prints version information

OPTIONS:
    -e, --entrypoint <entrypoint>     [default: main]
    -f, --file <file>
    -b, --heap-base <heap_base>       [default: 65536]
```

Example:

```sh
was -f app.wasm
```

The `--canary-check-on-alloc` option checks every single canary before
every single application. This is slow, and will get slower as the
number of allocation grows.

The `--heap-base` option sets how much data is already present on the
heap before dynamic allocations are performed. This is typically used
to store static data. When using AssemblyScript, the optimal value for
the heap base is stored in the `HEAP_BASE` global.

## Usage with AssemblyScript

WAS (not WASM) was originally made to work with AssemblyScript.

In order to do so, use the `memory` allocator:

```typescript
import 'allocator/system';
```

Optionally, in order to check canaries when the application
terminates, call the `terminate()` function in your `index.ts` file:

```typescript
declare function terminate(): void;

@global export function main(): void {
   ...
   terminate();
}
```

AssemblyScript stores static data at the beginning of the heap. The
heap base after this static data, stored in the `HEAP_BASE` global.

A quick way to print it while using WAS (not WASM) is to temporarily
add this to your application:

```typescript
declare function debug_val(val: u32): void;

debug_val(HEAP_BASE);
```
