use crate::{vm::*, *};

pub type NativeFn = fn(&mut crate::VM) -> Result<(), crate::RuntimeError>;

pub fn create_rust_function(function: NativeFn) -> Chunk {
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
    
    /// Gets an function index out of the desired index from the call stack
    pub fn get_function_idx(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);
        
        // Type check
        if let Variable::Function(func) = var  {
            Ok(func)
        } else {
            Err(RuntimeError::TypeError(format!("expected function, got {}", var.type_name())))
        }
    }

    /// Gets an table index out of the desired index from the call stack
    pub fn get_table_idx(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);
        
        // Type check
        if let Variable::Table(tb) = var  {
            Ok(tb)
        } else {
            Err(RuntimeError::TypeError(format!("expected table, got {}", var.type_name())))
        }
    }

    pub fn get_userdata_idx(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);
        
        // Type check
        if let Variable::Userdata(tb) = var  {
            Ok(tb)
        } else {
            Err(RuntimeError::TypeError(format!("expected userdata, got {}", var.type_name())))
        }
    }

    /// Gets a coroutine index out of the desired index from the call stack
    pub fn get_coroutine_idx(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);
        
        // Type check
        if let Variable::Coroutine(co) = var  {
            Ok(co)
        } else {
            Err(RuntimeError::TypeError(format!("expected coroutine, got {}", var.type_name())))
        }
    }

    /// Gets a string index out of the desired index from the call stack
    pub fn get_string_idx(&self, index: usize) -> Result<usize, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);
        
        // Type check
        if let Variable::String(str) = var  {
            Ok(str)
        } else {
            Err(RuntimeError::TypeError(format!("expected string, got {}", var.type_name())))
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
            Err(RuntimeError::TypeError(format!("expected number, got {}", var.type_name())))
        }
    }

    pub fn get_f64(&self, index: usize) -> Result<f64, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);

        // Type check
        if let Variable::Float(f) = var  {
            Ok(f)
        } else {
            Err(RuntimeError::TypeError(format!("expected number, got {}", var.type_name())))
        }
    }

    pub fn get_bool(&self, index: usize) -> Result<bool, crate::RuntimeError> {
        let var = self.call_stack.get(index).copied().unwrap_or(Variable::Nil);

        // Type check
        if let Variable::Boolean(b) = var  {
            Ok(b)
        } else {
            Err(RuntimeError::TypeError(format!("expected boolean, got {}", var.type_name())))
        }
    }

    /// Creates and returns a new VM function using a `NativeFn`
    pub fn new_function(&mut self, func: NativeFn) -> Variable {
        Variable::Function(self.functions.insert(create_rust_function(func)))
    }

    /// Creates and returns a new Table
    pub fn new_table(&mut self, table: Table) -> Variable {
        Variable::Table(self.tables.insert(table))
    }

    /// Creates and returns a VM string
    pub fn new_string(&mut self, str: &str) -> Variable {
        Variable::String(self.intern_string(str))
    }

    pub fn new_userdata(&mut self, data: Userdata) -> Variable {
        Variable::Userdata(self.userdatas.insert(data))
    }
}