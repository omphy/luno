# luno

`luno` is an experimental compiler and runtime, aiming to run a Lua 5.1 dialect language fully implemented in pure Rust. The main goal of the project is to provide a Lua-like runtime that has absolutely no external language dependencies.

## Stability
Currently the API *and* written code is completely unstable and can change at any time. While normal Lua code should be mostly stable, custom features like the `global` keyword and `[1, 2, 3]` arrays could be changed or removed entirely if weird edge cases are found with the current implementation or if it's decided that the feature is completely unecessary.
Additionally code is prone to crashing and there are lots of other issues and unfinished features that still need to be ironed out.

Therefore it is currently **strongly not** recommended to be used in any production programs.

## Safety
Almost all of the code is written in fully safe Rust as safe code is the priority. However if unsafe code can provide a speed up or help with other features it could still be used.
But unlike other code, most of the experimental JIT related code is inherently almost fully unsafe Rust. 

## Usage
### Running code
Running code is very straightforward. You create a new Lua runtime and you can simply call `execute` on it with the inputted code.
```rust
use luno_compiler::Lua;

// Create a new Lua runtime
let mut lua = Lua::new();

// Run code and check if there was a runtime error
if let Err(error) = lua.execute(include_str!("./example.lua")) {
    println!("{error}");
}
```
### Installation
Currently, there is no crates.io page for either the compiler or runtime. More information will be included later.

## Goals
### Planned
* Have a runtime speed comparable, if not better than PUC-Rio Lua. Or at the very least not being dreadfully slow.
* Providing an easy to use Rust API.
* Being mostly compatible with Lua 5.1.
* Support many compile targets like `wasm`.
* Have optional JIT support using `cranelift` to provide a massive speed up in runtime performance.
* Including a compiler and runtime that can perform smart optimizations and eventually be used as a compile target for multiple languages.
* Having clear error messages, similar to those of Rust.

### Considered
* Full Lua 5.1 compatibility
* `ffi` functionality like LuaJIT.
* Actual arrays `[1, 2, 3]` though this syntax conflicts with Lua's multiline string syntax.
* `global` as a keyword

### Not planned
* Be designed to run on microcontrollers or embedded systems. The runtime and compiler are intended as a scripting language for games or executables.
* Having super fast compile times. While the goal is to keep compile times fast enough for the language to be compiled at runtime, it will probably not be as fast as Lua.
* Be a completely focused on sandboxed execution like for example Luau is. Though ensuring untrusted scripts can still run safely if you set up the environment for it *is* planned.