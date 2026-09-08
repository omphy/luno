use luno_compiler::Lua;
use std::time::{Duration, Instant};

fn main() {
    let mut lua = Lua::new();
    
    let context = lua.compile(include_str!("./example.lua")).expect("Failed to compile script!");
    context.chunk.disassemble("MainChunk");

    println!("\nRUNTIME OUTPUT");
    let runs = 1;
    let mut total_duration = Duration::ZERO;

    for _ in 0..runs {
        let start_time = Instant::now();

        if let Err(err) = lua.execute_context(&context) {
            eprintln!("{err}");
        }
        total_duration += start_time.elapsed();
    }

    let average_duration = total_duration / runs;
    println!("Average execution was {:?} over {} runs", average_duration, runs);
}
