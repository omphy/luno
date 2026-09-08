mod ast;
mod compiler;
mod parser;
mod token;

use runtime::errors::CompileError;
// pub use runtime::vm_function;
pub use runtime::rust_functions;
pub use runtime::vm;

use compiler::Compiler;
use parser::parse_code;
use runtime::vm::Table;
use runtime::vm::Variable;
use runtime::{Runtime, Language, SharedContext, standard_lib};

pub struct Lua;

impl Lua {
    pub fn add_library(runtime: &mut Runtime) {
        // Print stuff
        runtime.vm.set_global_function("print", standard_lib::vm_print);
        
        // Coroutines
        let mut coroutine = Table::new();
        let new = Variable::String(runtime.vm.intern_string("new"));
        coroutine.set(new, Variable::Float(5.5));
        runtime.vm.set_global("coroutine", coroutine);
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
