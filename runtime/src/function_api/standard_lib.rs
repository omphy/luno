use crate::{vm::{self, *}, *};

/// Turns a VM variable into a string
pub fn var_tostring(vm: &crate::VM, var: &vm::Variable) -> String {
    match var {
        Variable::Float(f) => format!("{f}"),
        Variable::Boolean(b) => format!("{b}"),
        Variable::String(s) => vm.strings[*s].to_string(),
        Variable::Function(func) => format!("function: {:p}", &vm.functions[*func]),
        Variable::Array(arr) => format!("array: {:p}", &vm.arrays[*arr]),
        Variable::Table(tb) => format!("table: {:p}", &vm.arrays[*tb]),
        Variable::Nil => "nil".to_string(), // truly some crazy stuff
    }
}

/// Turns every input VM variable into a string and prints it to the console with a `\t` in between
pub fn vm_print(vm: &mut crate::VM) -> Result<(), RuntimeError> {
    let mut output = String::new();

    for (i, var) in vm.call_stack.iter().enumerate() {
        if i > 0 {
            output += "\t"; 
        }

        output += &var_tostring(vm, var);
    }

    #[cfg(not(target_arch = "wasm32"))]
    println!("{}", output);

    #[cfg(target_arch = "wasm32")]
    log::info!("{}", output);
    Ok(())
}

/// Sets the index of the array to a specific value
pub fn array_set(vm: &mut crate::VM) -> Result<(), RuntimeError> {
    // arr: usize, index: usize, var: Variable
    let array_idx: usize = vm.get_array_idx(0)?;
    let index = vm.get_usize(1)? - 1;
    let var = vm.get_any(2);

    if let Some(array) = vm.arrays.get_mut(array_idx) {
        array.set(index, var);
        Ok(())
    } else {
        Err(RuntimeError::InternalError(
            format!("Invalid array handle {array_idx}: slot is vacant or out of bounds")
        ))
    }
}