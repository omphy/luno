use crate::{rust_functions::{NativeFn, create_rust_function}, vm::{Variable, *}, *};

pub trait IntoVariable {
    fn into_variable(self, vm: &mut VM) -> Variable;
}

// Generics
impl IntoVariable for f64 {
    fn into_variable(self, _vm: &mut VM) -> Variable {
        Variable::Float(self)
    }
}

// Heap objects
impl IntoVariable for Chunk {
    fn into_variable(self, vm: &mut VM) -> Variable {
        // Allocate space to store it
        let heap_index = vm.functions.insert(self);
        Variable::Function(heap_index)
    }
}

impl IntoVariable for Table {
    fn into_variable(self, vm: &mut VM) -> Variable {
        // Allocate space to store it
        let heap_index = vm.tables.insert(self);
        Variable::Table(heap_index)
    }
}

impl VM {
    // Set global for everything
    pub fn set_global<T: IntoVariable>(&mut self, name: impl Into<String>, value: T) {
        let vm_val = value.into_variable(self);

        let name = name.into();
        self.globals.insert(name, vm_val);
    }

    // Set global for functions
    pub fn set_global_function(&mut self, name: impl Into<String>, function: NativeFn) {
        self.set_global(name, create_rust_function(function));
    }
}