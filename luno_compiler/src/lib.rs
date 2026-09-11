mod ast;
mod compiler;
mod parser;
mod token;

use runtime::errors::CompileError;
pub use runtime::rust_functions;
pub use runtime::vm;

use compiler::Compiler;
use parser::parse_code;
use runtime::vm::Table;
use runtime::vm::Variable;
use runtime::{Runtime, Language, SharedContext, standard_lib};

pub struct Lua;

// test function
pub fn callback_test(vm: &mut vm::VM) -> Result<(), runtime::errors::RuntimeError> {
    let fn_var = vm.call_stack.first().copied().unwrap_or(Variable::Nil);

    for i in 1..=10 {
        vm.call(fn_var, &[Variable::Float(i as f64)])?;
    }

    Ok(())
}

impl Lua {
    pub fn add_library(runtime: &mut Runtime) {
        // Print stuff
        runtime.vm.set_global_function("print", standard_lib::vm_print);
        
        // Coroutines
        let mut coroutine = Table::new();

        // .create()
        let create_function = runtime.vm.new_function(standard_lib::vm_co_create);
        coroutine.set_field(&mut runtime.vm, "create", create_function);
        
        // .resume()
        let resume_function = runtime.vm.new_function(standard_lib::vm_co_resume);
        coroutine.set_field(&mut runtime.vm, "resume", resume_function);
        
        // .yield()
        let yield_function = runtime.vm.new_function(standard_lib::vm_co_yield);
        coroutine.set_field(&mut runtime.vm, "yield", yield_function);

        runtime.vm.set_global("coroutine", coroutine);

        // test function
        runtime.vm.set_global_function("for10", callback_test);
    }

    pub fn new() -> Runtime {
        let mut runtime = Runtime::new();
        Lua::add_library(&mut runtime);

        runtime.add_language("lua", Self);
        runtime.set_main_language("lua");

        runtime
    }
}

impl Language for Lua {
    fn compile(
        &self,
        source: &str,
        context: &mut SharedContext,
        _runtime: &Runtime,
    ) -> Result<(), CompileError> {
        let program = parse_code(source);

        // println!("\n----- AST -----");
        // println!("{program:#?}\n");

        let mut compiler = Compiler::new();

        context.chunk = compiler.compile(program)?;

        Ok(())
    }
}
