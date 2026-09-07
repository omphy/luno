use crate::{vm::*, *};

pub fn set_index<T: Default + Clone>(vec: &mut Vec<T>, index: usize, value: T) {
    if index >= vec.len() {
        vec.resize(index + 1, Default::default());
    }
    vec[index] = value;
}

impl VM {
    // region:helpers
    /// Set a register to a desired value and safely resize if necessary, adjusted for the call stack
    pub(super) fn set_register(&mut self, register: usize, var: Variable) {
        let base = self.frames.last().map(|f| f.register_base).unwrap_or(0);
        let absolute_reg = base + register;

        if absolute_reg >= self.registers.len() {
            self.registers.resize_with(absolute_reg + 1, || Variable::Nil);
        }
        self.registers[absolute_reg] = var;
    }

    /// Get the register, adjusted for the call stack
    pub(super) fn get_register(&self, register: u8) -> Variable {
        let base = self.frames.last().map(|f| f.register_base).unwrap_or(0);
        self.registers.get(base + register as usize).copied().unwrap_or(Variable::Nil)
    }

    /// Gets the index of a function on the heap from the register
    pub(super) fn get_function_index(&self, register: u8) -> Result<usize, RuntimeError> {
        let var = self.get_register(register);
        match var {
            Variable::Function(idx) => Ok(idx),
            _ => Err(RuntimeError::TypeError(format!("expected function, got {}", var.type_name())))
        }
    }

    /// Interns the string
    pub fn intern_string(&mut self, s: &str) -> usize {
        // Check if the string was already interned
        if let Some(&idx) = self.interned_strings.get(s) {
            return idx;
        }

        // If it wasnt, create a new string and intern it
        let idx = self.strings.insert(s.to_string());
        self.interned_strings.insert(s.to_string(), idx);
        idx
    }

    /// Turns the number into a string, if its already a string it simply returns the string, otherwise it raises an error.
    pub fn num_to_str(&mut self, val: Variable) -> Result<String, RuntimeError> {
        match val {
            Variable::Float(f) => Ok(f.to_string()),
            Variable::String(str_idx) => {
                if let Some(string) = self.strings.get(str_idx) {
                    Ok(string.to_string())
                } else {
                    Err(RuntimeError::InternalError(
                        format!("Invalid array handle {str_idx}: slot is vacant or out of bounds")
                    ))
                }
            }
            _ => Err(RuntimeError::ConversionError(format!("attempted to convert {} to string", val.type_name())))
        }
    }
    // endregion:helpers
}