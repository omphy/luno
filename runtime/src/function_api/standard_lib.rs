use crate::{vm::{self, *}, *};

/// Turns a VM variable into a string
pub fn var_tostring(vm: &crate::VM, var: &vm::Variable) -> String {
    match var {
        Variable::Float(f) => format!("{f}"),
        Variable::Boolean(b) => format!("{b}"),
        Variable::String(s) => vm.strings[*s].to_string(),
        Variable::Function(func) => format!("function: {:p}", &vm.functions[*func]),
        Variable::Array(arr) => format!("array: {:p}", &vm.arrays[*arr]),
        Variable::Table(tb) => format!("table: {:p}", &vm.tables[*tb]),
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