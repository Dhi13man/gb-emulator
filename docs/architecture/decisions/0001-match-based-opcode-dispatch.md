---
decision_date: 2025-01-01
pattern: match-based-opcode-dispatch
repository: gb-emulator
status: Discovered
tags: [adr, cpu, performance, dispatch]
title: "0001. Match-Based Opcode Dispatch"
type: adr
---

# 0001. Match-Based Opcode Dispatch

Date: 2025-01-01

## Status

Discovered (existing architecture)

## Context

The SM83 CPU has 256 standard opcodes and 256 CB-prefixed opcodes (512 total). Each opcode must be decoded and executed on every CPU step. The dispatch mechanism directly impacts emulation performance since it runs millions of times per second.

Common approaches in emulator design:

1. **Function pointer table**: Array of 256 function pointers, indexed by opcode
2. **Match statement**: Exhaustive match on opcode byte
3. **Computed goto**: Language-specific jump table (not idiomatic Rust)

Evidence:

- `gb-core/src/cpu/opcodes.rs`: Single `execute()` method with 256-arm match statement
- `gb-core/src/cpu/cb_opcodes.rs`: Single `execute_cb()` method with 256-arm match statement
- `gb-core/src/cpu/mod.rs:73-78`: Dispatch via `if opcode == 0xCB { execute_cb } else { execute }`

## Decision

The system uses a **match-based dispatch** pattern where each opcode is a literal arm in a Rust `match` expression. The CB-prefix is handled as a separate match in a dedicated method.

## Consequences

**Benefits**:

- The Rust compiler optimizes exhaustive match on `u8` into a jump table, achieving equivalent performance to function pointer arrays
- All opcode logic is visible in a single file per category, aiding debugging and correctness auditing
- No runtime indirection or vtable overhead
- Exhaustiveness is compiler-enforced: missing an opcode arm is a compile error
- Each arm can inline register access and ALU helpers without function call overhead

**Trade-offs**:

- `opcodes.rs` and `cb_opcodes.rs` are large files (~256 arms each), which can be harder to navigate
- Adding instrumentation (e.g., opcode frequency counting) requires modifying the match body rather than wrapping a function pointer
- No runtime opcode patching or hot-swapping (not needed for a static ISA)
