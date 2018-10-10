# Ruinfuck

An optimizing brainfuck interpreter written in Rust.

## Running

Run a brainfuck script from a file:

```
cargo run --release fuck/hello.fuck
```

Run the brainfuck repl.

```
cargo run --release
```

## Implemented Optimizations

- Merge Repeated Operators
- Collapse Assignments
- Collapse Offsets
- Defer Movements
- Collapse Simple Moves
- Collapse Scanloops


## Interesting Reads

- http://calmerthanyouare.org/2015/01/07/optimizing-brainfuck.html