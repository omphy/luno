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
use runtime::{Runtime, Language, SharedContext, standard_lib};

pub struct Lua;

impl Lua {
    pub fn add_library(runtime: &mut Runtime) {
        // Print stuff
        runtime.vm.set_global_function("print", standard_lib::vm_print);

        // // Array stuff
        runtime.vm.set_global_function("array_set".to_string(), standard_lib::array_set);
        // runtime.vm.push_global_function("array_append".to_string(), create_rust_function(standard_lib::array_append));
        // runtime.vm.push_global_function("array_get".to_string(), create_rust_function(standard_lib::array_get));
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
