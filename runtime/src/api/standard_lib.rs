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
        Variable::Coroutine(tb) => format!("thread: {:p}", &vm.coroutines[*tb]),
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

/// Create a new coroutine with a function as input
pub fn vm_co_create(vm: &mut VM) -> Result<(), RuntimeError> {
    let func_idx = vm.get_function_idx(0)?;
    
    let mut co = Coroutine {
        registers: Vec::new(),
        frames: Vec::new(),
        state: CoroutineState::Suspended,
        caller: None,
    };
    
    // Push base call frame
    co.frames.push(CallFrame {
        func_idx,
        ip: 0,
        register_base: 0,
        from_rust: true,
    });
    
    let co_idx = vm.coroutines.insert(co);
    
    // Return the coroutine
    vm.return_values = vec![Variable::Coroutine(co_idx)];
    Ok(())
}

/// Resumes the inputted coroutine
pub fn vm_co_resume(vm: &mut VM) -> Result<(), RuntimeError> {
    let co_idx = vm.get_coroutine_idx(0)?;

    let co = vm.coroutines.get_mut(co_idx).unwrap();
    if co.state == CoroutineState::Dead {
        return Err(RuntimeError::ThreadError("cannot resume dead coroutine".into()));
    }

    co.caller = Some(vm.current_co);
    co.state = CoroutineState::Running;

    let args = vm.call_stack[1..].to_vec();
    
    if co.registers.is_empty() {
        // First time code resumed, so push arguments
        co.registers.extend(args);
    } else {
        vm.return_values = args;
    }

    vm.swap_context(co_idx);
    Ok(())
}

/// Yield the inputted coroutine and return inputted values
pub fn vm_co_yield(vm: &mut VM) -> Result<(), RuntimeError> {
    let caller = match vm.coroutines[vm.current_co].caller {
        Some(c) => c,
        None => return Err(RuntimeError::ThreadError("attempt to yield from outside a coroutine".into())),
    };

    if vm.native_depth > 1 {
        return Err(RuntimeError::ThreadError(
            "attempt to yield across a native call boundary".to_string()
        ));
    }

    vm.coroutines[vm.current_co].state = CoroutineState::Suspended;
    
    // Pass everything yielded from the call stack to the caller's call stack
    vm.return_values = vm.call_stack.clone();

    vm.swap_context(caller);
    Ok(())
}