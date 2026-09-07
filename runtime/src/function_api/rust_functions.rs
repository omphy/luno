use crate::{vm::Variable, *};

pub type NativeFn = fn(&mut crate::VM) -> Result<(), crate::RuntimeError>;

pub fn create_rust_function(function: fn(&mut crate::VM) -> Result<(), crate::RuntimeError>) -> Chunk {
    let mut chunk = Chunk::new();
    chunk.rust_function = Some(function);
    chunk
}

impl VM {
    /// Gets `Variable` out of the desired index from the call stack
    pub fn get_any(&self, index: usize) -> Variable {
        self.call_stack.get(index).copied().unwrap_or(Variable::Nil)
    }
    
    /// Gets an array index out of the desired index from the call stack
    pub fn get_array_idx(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);
        
        // Type check
        if let Variable::Array(arr) = var  {
            Ok(arr)
        } else {
            Err(RuntimeError::TypeError(format!("expected array, got {}", var.type_name())))
        }
    }
    
    // CONSIDER: Might want to do a bounds check on this one and error if it doesnt pass?
    /// Gets a usize out of the desired index from the call stack
    pub fn get_usize(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);

        // Type check
        if let Variable::Float(f) = var  {
            Ok(f as usize)
        } else {
            Err(RuntimeError::TypeError(format!("expected array, got {}", var.type_name())))
        }
    }
}