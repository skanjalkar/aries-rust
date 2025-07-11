# ARIES-Rust: A Database Recovery Protocol Implementation

Ever wondered how databases magically recover from crashes without losing your data? Meet ARIES - the battle-tested algorithm that powers most modern database systems. This project is a Rust implementation of the ARIES recovery protocol, built from the ground up to be safe, fast, and actually understandable.

## What is ARIES?

ARIES (Algorithm for Recovery and Isolation Exploiting Semantics) is the recovery protocol used by databases like IBM DB2, Microsoft SQL Server, and many others. It's the reason your database can crash mid-transaction and still wake up with all your data intact. Pretty neat, right?

This implementation focuses on three core principles:
- **Write-Ahead Logging**: All changes are logged before they hit the disk
- **Repeating History**: During recovery, we replay exactly what happened
- **Logging Changes**: Even during recovery, we log what we're doing

## Why Rust?

Good question! First reason is that I wanted to play with Rust. While most database implementations are in C or C++, Rust gives us:
- **Memory safety** without garbage collection overhead
- **Fearless concurrency** - perfect for database workloads
- **Zero-cost abstractions** - performance without sacrificing readability
- **Excellent error handling** - databases can't just panic and call it a day

This project is inspired from BuzzDB, used in Georgia Techs DB Lab.

## Project Structure

```
src/
├── buffer/         # Buffer pool management (pages in memory)
├── common/         # Shared types and error handling
├── heap/           # Heap file management (where data lives)
├── log_mod/        # Write-ahead logging implementation
├── storage/        # Slotted page storage format
└── transaction/    # Transaction management and ACID guarantees
```

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Basic understanding of databases (helpful but not required)

### Building

```bash
git clone https://github.com/skanjalkar3/aries-rust
cd aries-rust
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test buffer_tests
cargo test recovery_tests
```

## Key Features

### ✅ What's Working

- **Buffer Management**: Efficient page caching with LRU replacement
- **Slotted Page Storage**: Variable-length record storage
- **Transaction Management**: ACID properties with proper isolation
- **Write-Ahead Logging**: All modifications are logged first
- **Crash Recovery**: Full ARIES recovery with Analysis, Redo, and Undo phases

### 🚧 What's Next

- **Performance Optimization**: Better buffer replacement policies
- **Concurrency**: More sophisticated locking mechanisms
- **Monitoring**: Performance metrics and debugging tools
- **Documentation**: More examples and tutorials


## Performance Notes

This isn't meant to compete with PostgreSQL or MySQL -- this is more for my learning :D. It's an educational implementation that prioritizes:
1. **Correctness** over raw performance
2. **Readability** over clever optimizations
3. **Learning** over production use


## Learning Resources

If you're new to database internals, check out:
- [Database Internals](https://www.databass.dev/) by Alex Petrov
- [Architecture of a Database System](https://dsf.berkeley.edu/papers/fntdb07-architecture.pdf)
- [The original ARIES paper](https://cs.stanford.edu/people/chrismre/cs345/rl/aries.pdf) (warning: it's dense!)

## Contributing

Found a bug? Want to add a feature? Great! This project is very much a work in progress. Please:

1. Check the existing issues
2. Write tests for your changes
3. Keep the code readable (we're learning here!)
4. Update documentation as needed

## License

MIT License - feel free to use this for learning, teaching, or whatever makes you happy.

## Acknowledgments

This project was inspired by the original ARIES paper and countless database textbooks. Special thanks to the Rust community for making systems programming fun again!

---

*"The best way to understand something is to build it from scratch."* - Someone wise, probably
