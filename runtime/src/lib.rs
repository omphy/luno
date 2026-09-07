// Setup modules
// (not idiomatic i know but i prefer this)

// Compiler
pub mod compile {
    pub mod bytecode;
    pub mod ir;
    pub mod ir_generator;
}
pub use compile::{bytecode, ir, ir_generator};

// Rust function api
pub mod function_api {
    pub mod rust_functions;
    pub mod standard_lib;
}
pub use function_api::{rust_functions, standard_lib};

// Rust api
pub mod api {
    pub mod globals;
}
pub use api::{globals};

// VM
pub mod vm {
    pub mod utils;
    pub mod vm;
    
    pub use self::vm::*;
}
pub use vm::utils;

// JIT compiler
#[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))] 
pub mod jit {
    pub mod jit;
    pub mod helper;
    pub mod memory;
    
    pub use self::jit::*;
}
#[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))] 
pub use jit::{helper, memory};

// Error handling
pub mod errors;

// paste
pub use paste;

//////////////////////////////////////////////////////////////////////////////

// pub use rust_functions::{Args, apply};
use bytecode::Chunk;
use std::collections::HashMap;
use vm::VM;

use crate::{errors::{CompileError, RuntimeError}, ir_generator::IrGenerator};

/// A compiler variable that is shared across all languages compiling into the same chunk.
#[derive(Clone, Debug)]
pub struct CompVar {
    pub name: String,
    pub scope: u32,
    pub register: u8,
}

/// The context containing the combined bytecode and the shared local comp variables.
#[derive(Default)]
pub struct SharedContext {
    pub ir: IrGenerator,
    pub chunk: Chunk,
    pub locals: Vec<CompVar>,
    pub scope_depth: u32,
    pub register_count: u8,
}

impl SharedContext {
    /// Grabs the next available register and increments the count.
    /// Used for temporaries.
    pub fn allocate_register(&mut self) -> u8 {
        let reg = self.register_count;
        self.register_count += 1;
        reg
    }

    /// Frees the most recently allocated register so it can be reused.
    pub fn free_register(&mut self) {
        if self.register_count > 0 {
            self.register_count -= 1;
        }
    }

    /// Allocates a register and binds a named variable to it.
    pub fn declare_variable(&mut self, name: String) -> u8 {
        let reg = self.allocate_register();
        self.locals.push(CompVar {
            name,
            scope: self.scope_depth,
            register: reg,
        });
        reg
    }

    /// Find a local variable by name.
    pub fn resolve_variable(&self, name: &str) -> Option<u8> {
        self.locals
            .iter()
            .rev()
            .find(|local| local.name == name)
            .map(|local| local.register)
    }

    /// Increase the scope depth
    pub fn push_scope(&mut self) {
        self.scope_depth += 1;
    }

    /// Decrease the scope depth
    pub fn pop_scope(&mut self) {
        self.scope_depth -= 1;
    }
}

/// The trait that any language must implement.
pub trait Language {
    /// Takes source code and compiles it into the shared context.
    fn compile(
        &self,
        source: &str,
        context: &mut SharedContext,
        interpreter: &Runtime,
    ) -> Result<(), CompileError>;
}

/// The main runtime of all the languages.
pub struct Runtime {
    // A registry of all languages added to the Runtime.
    // e.g., "lua" -> LuaFrontend
    languages: HashMap<String, Box<dyn Language>>,

    // The name of the default language to use if no tags are provided.
    main_language: Option<String>,

    pub vm: VM,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            languages: HashMap::new(),
            main_language: None,
            vm: VM::new(),
        }
    }

    /// For building, if changing at runtime use `set_jit` instead
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))] 
    pub fn enable_jit(mut self, enabled: bool) -> Self {
        self.vm.jit_enabled = enabled;
        self
    }

    /// Just-in-time compilation is not supported on your architecture.
    #[cfg(any(target_arch = "wasm32", not(target_pointer_width = "64")))]
    #[deprecated(note = "JIT compilation is not supported on web or 32-bit architectures.")]
    pub fn enable_jit(self, enabled: bool) -> Self {
        if enabled {
            eprintln!("JIT is not supported on this architecture. Falling back to the interpreter.");
        }
        self
    }

    /// Just-in-time compilation is disabled. Enable the `jit` feature to use it.
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", not(feature = "jit")))]
    #[deprecated(note = "The 'jit' feature is disabled. Enable it in your Cargo.toml to use JIT compilation.")]
    pub fn enable_jit(self, enabled: bool) -> Self {
        if enabled {
            eprintln!("The 'jit' feature is disabled. Falling back to the interpreter.");
        }
        self
    }
    
    /// For changing JIT at runtime, if toggling when building use `enable_jit` instead
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))] 
    pub fn set_jit(&mut self, enabled: bool) {
        self.vm.jit_enabled = enabled;
    }

    /// Just-in-time compilation is not supported on your architecture.
    #[cfg(any(target_arch = "wasm32", not(target_pointer_width = "64")))]
    #[deprecated(note = "JIT compilation is not supported on web or 32-bit architectures.")]
    pub fn set_jit(&mut self, enabled: bool) {
        if enabled {
            eprintln!("JIT is not supported on this architecture. Falling back to the interpreter.");
        }
    }

    /// Just-in-time compilation is disabled. Enable the `jit` feature to use it.
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", not(feature = "jit")))]
    #[deprecated(note = "The 'jit' feature is disabled. Enable it in your Cargo.toml to use JIT compilation.")]
    pub fn set_jit(&mut self, enabled: bool) {
        if enabled {
            eprintln!("The 'jit' feature is disabled. Falling back to the interpreter.");
        }
    }

    /// Add a language to the interpreter.
    pub fn add_language(&mut self, name: &str, language: impl Language + 'static) -> &mut Self {
        self.languages.insert(name.to_string(), Box::new(language));
        self
    }

    /// Set which language should act as the host/default.
    pub fn set_main_language(&mut self, name: &str) -> &mut Self {
        if !self.languages.contains_key(name) {
            panic!(
                "Cannot set main language to '{}': language not registered.",
                name
            );
        }
        self.main_language = Some(name.to_string());
        self
    }

    /// Compiles the source code and returns the SharedContext (AST -> Bytecode)
    pub fn compile(&self, source: &str) -> Result<SharedContext, CompileError> {
        let main_lang_name = self.main_language.as_ref().expect("No main language set!");
        let mut context = SharedContext::default();

        self.compile_guest(main_lang_name, source, &mut context)?;

        Ok(context)
    }

    /// Compiles a foreign block of code directly into an existing context
    pub fn compile_guest(
        &self,
        lang_name: &str,
        source: &str,
        context: &mut SharedContext,
    ) -> Result<(), CompileError> {
        let compiler = self
            .languages
            .get(lang_name)
            .ok_or_else(|| CompileError::NotFound(format!("language '{}' not found in registry", lang_name)))?;

        compiler.compile(source, context, self)
    }

    /// Takes a pre-compiled SharedContext and runs it in the VM
    pub fn execute_context(&mut self, context: &SharedContext) -> Result<(), RuntimeError> {
        self.vm.execute(context.chunk.clone())
    }

    /// Compiles and runs the given source code.
    pub fn execute(&mut self, source: &str) -> Result<(), RuntimeError> {
        match self.compile(source) {
            Ok(context) => {
                self.execute_context(&context)
            }
            Err(e) => {
                panic!("Compilation Error: {}", e);
            }
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
