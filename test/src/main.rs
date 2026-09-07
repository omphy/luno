use luno_compiler::{Lua};
use std::time::{Duration, Instant};

// use runtime::{ir::IrInstruction::*, ir_generator::IRGenerator};

fn main() {
    // let mut genr = IRGenerator::new();
    // let ir = vec![
    //     Local(genr.intern("var1")),
    //     Local(genr.intern("var2")),
        
    //     LoadFloat { dest: genr.intern("var1"), value: 5.0 },
    //     LoadFloat { dest: genr.intern("var2"), value: 5.0 },
    //     Add { dest: genr.intern("var1"), left: genr.intern("var1"), right: genr.intern("var2") }
    // ];
    // genr.compile(ir)
    // .map(|val| val.disassemble("MainChunk"))
    // .unwrap_or_else(|err| eprintln!("{err}"));
        
    let mut lua = Lua::new();
    let source = include_str!("./example.lua");
    
    println!("--- Running Script: DEBUG ---");
    
    let context = lua.compile(source).expect("Failed to compile script!");
    context.chunk.disassemble("MainChunk");

    println!("\n--- RUNTIME OUTPUT ---");
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
    println!("Average execution took (over {} runs): {:?}", runs, average_duration);
}
