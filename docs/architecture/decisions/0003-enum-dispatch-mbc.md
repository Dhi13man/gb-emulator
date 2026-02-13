---
decision_date: 2025-01-01
pattern: enum-dispatch-cartridge
repository: gb-emulator
status: Discovered
tags: [adr, cartridge, mbc, polymorphism]
title: "0003. Enum Dispatch for Cartridge MBC"
type: adr
---

# 0003. Enum Dispatch for Cartridge MBC

Date: 2025-01-01

## Status

Discovered (existing architecture)

## Context

Game Boy cartridges use different Memory Bank Controllers (MBC) to extend the 32 KiB ROM address space. The emulator must support multiple MBC types (NoMBC, MBC1, MBC2, MBC3, MBC5) with a common interface for read/write operations. This is the classic polymorphism problem.

Common approaches in Rust:

1. **Trait objects** (`Box<dyn Cartridge>`): Dynamic dispatch via vtable
2. **Enum dispatch**: Enum wrapping concrete types, match in each method
3. **Generics**: `GameBoy<C: CartridgeTrait>` with monomorphization

Evidence:

- `gb-core/src/cartridge/mod.rs`: `Cartridge` enum with variants `NoMbc(NoMbc)`, `Mbc1(Mbc1)`, `Mbc2(Mbc2)`, `Mbc3(Mbc3)`, `Mbc5(Mbc5)`
- Methods `read()`, `write()`, `read_ram()`, `write_ram()` implemented via match on enum variants
- Each MBC in its own file: `no_mbc.rs`, `mbc1.rs`, `mbc2.rs`, `mbc3.rs`, `mbc5.rs`

## Decision

The system uses **enum dispatch** where `Cartridge` is an enum whose variants wrap concrete MBC structs. Each bus operation matches on the variant and delegates to the inner type.

## Consequences

**Benefits**:

- Zero-cost abstraction: the compiler can inline through the match, eliminating vtable overhead
- Compatible with `no_std` (no heap allocation for `Box<dyn Trait>`)
- The set of MBC types is closed and known at compile time (5 variants)
- Pattern is exhaustive: adding a new MBC variant without implementing all methods is a compile error
- Single allocation: the entire cartridge (ROM + RAM + registers) lives in one enum

**Trade-offs**:

- Adding a new MBC type requires modifying the enum definition and all match arms
- Each method has a 5-arm match (minimal overhead, but more boilerplate than trait objects)
- Not extensible by external crates (closed enum), though this is not a requirement
