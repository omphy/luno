use bytecode::Opcode::{self, *};
use std::ops::{Deref, DerefMut};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use rustc_hash::FxHashMap;
use slab::Slab;
use crate::*;

// CONSIDER: make the error text equal to lua's errors, this might not be done because I want to have better errors, even if it breaks compat with anything that reads the errors

/// Instruction layout (32-bit):
/// - 3-register format: [ 8-bit Opcode | 8-bit Target (A) | 8-bit Arg (B) | 8-bit Arg (C) ]
/// - Immediate format:  [ 8-bit Opcode | 8-bit Target (A) | 16-bit Constant Index (BC)   ]

/// Base variable type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Variable {
    /// 64 bit floating point number.
    Float(f64),

    /// Boolean, true/false
    Boolean(bool),

    // Heap variables, these point to an index on the heap rather than storing it themselves
    /// Contains a pointer to a heap allocated function/chunk
    Function(usize),

    /// Contains a pointer to a heap allocated array
    Array(usize),

    /// Contains a pointer to a heap allocated table
    Table(usize),

    /// Contains a pointer to a heap allocated string
    String(usize),

    /// Contains a pointer to a heap allocated coroutine
    Coroutine(usize),

    /// Represents nothing, also known as null and none
    Nil,
}

// Hash for floats
impl Eq for Variable {}

impl Hash for Variable {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            Variable::Float(f) => {
                state.write_u8(0);
                let bits = (if f == 0.0 { 0.0 } else { f }).to_bits();
                state.write_u64(bits);
            }
            Variable::Boolean(b) => {
                state.write_u8(1);
                state.write_u8(b as u8);
            }
            Variable::Function(i) => {
                state.write_u8(2);
                state.write_usize(i);
            }
            Variable::Array(i) => {
                state.write_u8(3);
                state.write_usize(i);
            }
            Variable::Table(i) => {
                state.write_u8(4);
                state.write_usize(i);
            }
            Variable::String(i) => {
                state.write_u8(5);
                state.write_usize(i);
            }
            Variable::Coroutine(i) => {
                state.write_u8(6);
                state.write_usize(i);
            }
            Variable::Nil => {
                state.write_u8(7);
            }
        }
    }
}

macro_rules! impl_relational_op {
    ($fn_name:ident, $op:tt) => {
        #[inline(always)]
        pub fn $fn_name(&self, other: &Self, vm: &VM) -> Result<bool, RuntimeError> {
            match (self, other) {
                (Variable::Float(a), Variable::Float(b)) => Ok(a $op b),
                (Variable::String(a), Variable::String(b)) => {
                    // String equality fast path for <= and >=
                    if a == b {
                        return Ok("" $op ""); // evaluates true for <=, false for <
                    }
                    let str_a = &vm.strings[*a];
                    let str_b = &vm.strings[*b];
                    Ok(str_a $op str_b)
                }
                _ => Err(RuntimeError::TypeError(format!("attempt to compare {} with {}", self.type_name(), other.type_name()))),
            }
        }
    };
}

impl Variable {
    pub fn type_name(&self) -> &'static str {
        match self {
            Variable::Float(_) => "number",
            Variable::Boolean(_) => "boolean",
            Variable::Function(_) => "function",
            Variable::String(_) => "string",
            Variable::Array(_) => "array",
            Variable::Table(_) => "table",
            Variable::Coroutine(_) => "thread",
            Variable::Nil => "nil",
        }
    }

    #[inline(always)]
    /// Checks if a variable is truthy according to VM rules
    pub fn is_truthy(&self) -> bool {
        match self {
            Variable::Nil => false,
            Variable::Boolean(b) => *b,
            _ => true, // Everything other than nil and false is truthy
        }
    }

    #[inline(always)]
    /// Checks if a variable is equal to another variable
    pub fn is_equal(&self, other: &Variable) -> bool {
        // This is a bit useless right now, this is because
        // its intended to prevent a rewrite if this ever does
        // do something. (e.g., string interning gets reworked)

        // Strings work because interning makes the pointers of 2 equal strings the same pointer.
        self == other
    }

    impl_relational_op!(less_than, <);
    impl_relational_op!(less_or_equal, <=);
}

/// Stores the current call frame for function calling
#[derive(Debug, Clone, Copy)]
pub struct CallFrame {
    /// The index of the function on the heap
    pub func_idx: usize,
    /// The Instruction Pointer
    pub ip: usize,
    /// Where this function's registers start in the global register array
    pub register_base: usize, 
    /// If this CallFrame was pushed from Rust
    pub from_rust: bool,
}

/// Runtime array struct, automatically handles bounds checking and returns proper variables 
pub struct Array(pub Vec<Variable>);

impl Array {
    fn new() -> Self {
        Self(Vec::new())
    }

    pub fn get(&self, index: usize) -> Variable {
        self.0.get(index).copied().unwrap_or(Variable::Nil)
    }

    pub fn set(&mut self, index: usize, var: Variable) {
        if index >= self.0.len() {
            // Resize vector and fill missing slots with nil
            self.0.resize(index + 1, Variable::Nil);
        }
        self.0[index] = var;
    }
}

// Derefs so that you dont have to type .0
impl Deref for Array {
    type Target = [Variable];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Array {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<Variable>> for Array {
    fn from(vec: Vec<Variable>) -> Self {
        Self(vec)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CoroutineState {
    Suspended,
    Running,
    Dead,
}

#[derive(Debug)]
pub struct Coroutine {
    pub registers: Vec<Variable>,
    pub frames: Vec<CallFrame>,
    pub state: CoroutineState,
    pub caller: Option<usize>,
}

/// Runtime table struct
#[derive(Default, Debug)]
pub struct Table {
    pub map: FxHashMap<Variable, Variable>,
    pub array: Vec<Variable>,
}

impl Table {
    pub fn new() -> Self {
        Default::default()
    }

    /// Get the variable stored at the index.
    pub fn get(&self, index: Variable) -> Variable {
        match index {
            Variable::Float(f) if f.fract() == 0.0 && f >= 0.0 => {
                let idx = f as usize;
                if idx < self.array.len() {
                    self.array[idx]
                } else {
                    self.map.get(&index).copied().unwrap_or(Variable::Nil)
                }
            } 
            _ => self.map.get(&index).copied().unwrap_or(Variable::Nil),
        }
    }

    /// Sets a variable on the table, but forces it to be on the hashmap. This is **not** recommended for normal usage as it can easily cause memory leaks.
    pub fn set_map(&mut self, index: Variable, var: Variable) {
        self.map.insert(index, var);
    }

    /// Sets the field to a variable.
    pub fn set_field(&mut self, vm: &mut VM, field: &str, var: Variable) {
        let idx = vm.new_string(field);
        self.set(idx, var);
    }

    /// Sets a variable on the table at the desired index.
    pub fn set(&mut self, index: Variable, var: Variable) {
        match index {
            Variable::Float(f) => {
                if f.fract() == 0.0 && f >= 0.0 {
                    let idx = f as usize;
                    let cap = self.array.capacity();

                    // Sparse indices
                    if idx > cap {
                        self.set_map(index, var);
                    } 
                    // Resize the array if its at max capacity
                    else if idx == cap {
                        self.array.reserve(1); 
                        self.array.resize(idx + 1, Variable::Nil);
                        self.array[idx] = var;

                        self.map.remove(&index);

                        for i in (cap + 1)..self.array.capacity() {
                            if let Some(migrated) = self.map.remove(&Variable::Float(i as f64)) {
                                if self.array.len() <= i {
                                    self.array.resize(i + 1, Variable::Nil);
                                }
                                self.array[i] = migrated;
                            }
                        }
                    } 
                    // Put it in the array
                    else {
                        if self.array.len() <= idx {
                            self.array.resize(idx + 1, Variable::Nil);
                        }
                        self.array[idx] = var;
                    }
                } else {
                    // Negatives and decimals
                    self.set_map(index, var);
                }
            }
            _ => self.set_map(index, var),
        }
    }
}

/// Virtual Machine
#[derive(Default)]
pub struct VM {
    /* Registers */
    /// Every global
    /// TODO: Make this a table when they are implented.
    pub globals: HashMap<String, Variable>,
    /// The call stack is where arguments are pushed on function calls, these are used specifically for the rust api.
    pub call_stack: Vec<Variable>,
    /// Every register stored on the VM
    pub registers: Vec<Variable>,
    /// Return values when a function returns
    pub return_values: Vec<Variable>,

    /// The call frames
    pub frames: Vec<CallFrame>,
    
    /* Heap */
    /// Every allocated function
    pub functions: Slab<Chunk>,

    /// Every allocated array
    pub arrays: Slab<Array>,

    /// Every allocated table
    pub tables: Slab<Table>,
    
    /// Every allocated string
    pub strings: Slab<String>,

    /// Every allocated coroutine
    pub coroutines: Slab<Coroutine>,
    /// The index of the currently executing coroutine
    pub current_co: usize,
    /// The depth of native calls
    pub native_depth: usize,

    /// String storage used by the string interner
    /// TODO: Optimize/replace/remove this
    pub interned_strings: FxHashMap<String, usize>,

    /// UNUSED: Intended for when JIT is implemented
    pub jit: bool,

    /// UNUSED: Intended for when things like spanning is implemented so theres better runtime error handling, 
    /// but you can turn it off for better performance
    pub debug: bool,
}

impl VM {
    /// Creates a new VM with default values and JIT disabled.
    pub fn new() -> Self {
        let mut vm = VM {
            debug: false,
            jit: false,
            ..Default::default()
        };
        
        // Setup the Main Thread at index 0
        let main_co = Coroutine {
            registers: Vec::new(),
            frames: Vec::new(),
            state: CoroutineState::Running,
            caller: None,
        };
        vm.current_co = vm.coroutines.insert(main_co);
        
        vm
    }

    // region:exec

    /// Swaps the frames from the coroutine and the VM
    pub fn swap_context(&mut self, target_co: usize) {
        let current_id = self.current_co;
        if current_id == target_co {
            return;
        }

        // Store the active state into the outgoing coroutine
        std::mem::swap(&mut self.registers, &mut self.coroutines[current_id].registers);
        std::mem::swap(&mut self.frames, &mut self.coroutines[current_id].frames);

        // Load the target coroutine's state into the VM
        std::mem::swap(&mut self.registers, &mut self.coroutines[target_co].registers);
        std::mem::swap(&mut self.frames, &mut self.coroutines[target_co].frames);

        self.current_co = target_co;
    }

    /// Execute a chunk on the VM.
    pub fn execute(&mut self, main_chunk: Chunk) -> Result<(), RuntimeError> {
        // Put the main chunk on the heap so the VM can reference it by index
        let main_idx = self.functions.insert(main_chunk);

        // Push the very first CallFrame
        self.frames.push(CallFrame {
            func_idx: main_idx,
            ip: 0,
            register_base: 0, // Starts at register 0
            from_rust: true,
        });

        self.run()
    }

    /// Start executing from a main function index
    pub fn run(&mut self) -> Result<(), RuntimeError> { 
        // Grab top frame
        loop {
            let mut frame = match self.frames.last().copied() {
                Some(f) => f,
                None => break,
            };

            let current_func_idx = frame.func_idx;

            // Execute instructions for this frame
            loop {
                let instruction = {
                    let chunk = match self.functions.get(current_func_idx) {
                        Some(c) => c,
                        _ => return Err(RuntimeError::InternalError("Invalid frame func_idx".to_string())),
                    };
                    
                    chunk.code[frame.ip]
                };
                
                frame.ip += 1;
                
                let opcode_byte = (instruction >> 24) as u8;
                let opcode: Opcode = unsafe { std::mem::transmute(opcode_byte) };

                // Math macro
                macro_rules! binary_op {
                    // Shorthand for standard operators (+, -, *, /, %)
                    ($op:tt) => {
                        binary_op!(|a, b| a $op b, stringify!($op))
                    };

                    // The core implementation that accepts any function/closure
                    ($func:expr, $name:expr) => {{
                        let target = (instruction >> 16) as u8;
                        let a = (instruction >> 8) as u8;
                        let b = instruction as u8;

                        let var_a = self.get_register(a);
                        let var_b = self.get_register(b);

                        match (var_a, var_b) {
                            (Variable::Float(f1), Variable::Float(f2)) => {
                                // Call the function/closure
                                let result = ($func)(f1, f2);
                                self.set_register(target as usize, Variable::Float(result));
                            }
                            (left, right) => {
                                return Err(RuntimeError::TypeError(format!(
                                    "Cannot perform binary operation '{}' on {} and {}",
                                    $name,
                                    left.type_name(),
                                    right.type_name()
                                )));
                            }
                        }
                    }};
                }

                match opcode {
                    /* LOAD OPCODES */
                    // region:load

                    LOADFLOAT => {
                        let target = (instruction >> 16) as u8;
                        let arg = instruction as u16;
                        
                        // Temporarily borrow the chunk to get the constant
                        let float_val = {
                            let chunk = match self.functions.get(current_func_idx) {
                                Some(c) => c,
                                _ => unreachable!(),
                            };
                            f64::from_bits(chunk.constants[arg as usize])
                        };
                        
                        self.set_register(target as usize, Variable::Float(float_val));
                    }

                    LOADSTR => {
                        let target = (instruction >> 16) as u8;
                        let arg = instruction as u16;

                        // Temporarily borrow the chunk to get the constant
                        let s = {
                            let chunk = match self.functions.get(current_func_idx) {
                                Some(c) => c,
                                _ => unreachable!(),
                            };
                            // TODO: get rid of .clone
                            chunk.string_constants[arg as usize].clone()
                        };

                        let str_idx = self.intern_string(&s);
                        self.set_register(target as usize, Variable::String(str_idx));
                    }

                    LOADFUNC => {
                        let target = (instruction >> 16) as u8;
                        let arg = instruction as u16;

                        // Create a temporary scope to immutably borrow the heap
                        // Then find the current chunk and clone the function out of it
                        let func_to_allocate = {
                            let chunk = match self.functions.get(current_func_idx) {
                                Some(c) => c,
                                _ => unreachable!(), // We know the current executing chunk should be valid
                            };
                            // TODO: get rid of .clone
                            chunk.functions[arg as usize].clone()
                        };

                        let func_idx = self.functions.insert(func_to_allocate);
                        self.set_register(target as usize, Variable::Function(func_idx));
                    }

                    LOADBOOL => {
                        let target = (instruction >> 16) as u8;
                        let arg = (instruction >> 8) as u8;

                        self.set_register(target as usize, Variable::Boolean(arg != 0));
                    }

                    LOADNIL => {
                        let target = (instruction >> 16) as u8;
                        self.set_register(target as usize, Variable::Nil);
                    }

                    // endregion:load
                    
                    // Binary ops
                    ADD => binary_op!(+),
                    MIN => binary_op!(-),
                    DIV => binary_op!(/),
                    MUL => binary_op!(*),
                    POW => binary_op!(f64::powf, "^"),
                    CONCAT => {
                        let target = (instruction >> 16) as u8;
                        let a = (instruction >> 8) as u8;
                        let b = instruction as u8;

                        let var_a = self.get_register(a);
                        let var_b = self.get_register(b);

                        let left = self.num_to_str(var_a)?;
                        let right = self.num_to_str(var_b)?;

                        let str_idx = self.intern_string(&(left + &right));
                        self.set_register(target as usize, Variable::String(str_idx));
                    }

                    MOV => {
                        let target = (instruction >> 16) as u8;
                        let arg = (instruction >> 8) as u8;

                        let var = self.get_register(arg);
                        self.set_register(target as usize, var);
                    }

                    // region:branching
                    
                    SKIP_ON_TRUE => {
                        let condition = (instruction >> 16) as u8;
                        if self.get_register(condition).is_truthy() {
                            frame.ip += 1; // Look how much cleaner and faster this is!
                        }
                    }
                    
                    JMP => {
                        let offset = ((instruction << 8) as i32) >> 8;
                        frame.ip = frame.ip.wrapping_add_signed((offset - 1) as isize); // No last_mut() needed!
                    }
                    
                    // endregion:branching
                    // region:comparison

                    EQ => {
                        let target = (instruction >> 16) as u8;
                        let left = (instruction >> 8) as u8;
                        let right = instruction as u8;

                        let equal = self.get_register(left).is_equal(&self.get_register(right));
                        self.set_register(target as usize, Variable::Boolean(equal));
                    }

                    NOTEQ => {
                        let target = (instruction >> 16) as u8;
                        let left = (instruction >> 8) as u8;
                        let right = instruction as u8;

                        let equal = self.get_register(left).is_equal(&self.get_register(right));
                        self.set_register(target as usize, Variable::Boolean(!equal));
                    }

                    LT => {
                        let target = (instruction >> 16) as u8;
                        let left = (instruction >> 8) as u8;
                        let right = instruction as u8;

                        let equal = self.get_register(left).less_than(&self.get_register(right), self)?;
                        self.set_register(target as usize, Variable::Boolean(equal));
                    }

                    LTEQ => {
                        let target = (instruction >> 16) as u8;
                        let left = (instruction >> 8) as u8;
                        let right = instruction as u8;

                        let equal = self.get_register(left).less_or_equal(&self.get_register(right), self)?;
                        self.set_register(target as usize, Variable::Boolean(equal));
                    }

                    // endregion:comparison
                    // region:globals

                    GET_GLOBAL => {
                        let target = (instruction >> 16) as u8;
                        let name_pos = instruction as u16;

                        // Temporarily borrow heap to get the string name
                        let name = {
                            let chunk = match self.functions.get(current_func_idx) {
                                Some(c) => c,
                                _ => unreachable!(),
                            };
                            &chunk.string_constants[name_pos as usize]
                        };
                        
                        // Get the variable, or return nil if it doesn't exist
                        let variable = self.globals.get(name).copied().unwrap_or(Variable::Nil);
                        self.set_register(target as usize, variable);
                    }

                    SET_GLOBAL => {
                        let value = (instruction >> 16) as u8;
                        let name_pos = instruction as u16;
                        
                        let name = {
                            let chunk = match self.functions.get(current_func_idx) {
                                Some(c) => c,
                                _ => unreachable!(),
                            };
                            &chunk.string_constants[name_pos as usize]
                        };
                        
                        self.globals.insert(name.to_string(), self.get_register(value));
                    }

                    // endregion:globals
                    // region:functions

                    CALL => {
                        let target = (instruction >> 16) as u8;
                        let start = (instruction >> 8) as u8;
                        let arg_count = instruction as u8;

                        // Save the current IP for returning
                        self.frames.last_mut().unwrap().ip = frame.ip;

                        // Grab the function to execute
                        let func_var = self.get_register(target);

                        // Collect arguments
                        let mut args = Vec::with_capacity(arg_count as usize);
                        for i in 0..arg_count {
                            let arg_reg = (start as u16 + i as u16) as u8;
                            args.push(self.get_register(arg_reg));
                        }

                        let context_changed = self.call(func_var, &args)?;
                        if context_changed {
                            break;
                        } else {
                            frame = *self.frames.last().unwrap();
                        }
                    }

                    RET => {
                        let start = (instruction >> 8) as u8;
                        let arg_count = instruction as u8;
                            
                        self.return_values.clear();
                        for i in 0..arg_count {
                            self.return_values.push(self.get_register(start + i));
                        }

                        let popped = self.frames.pop().unwrap();
                        self.registers.truncate(popped.register_base);

                        if self.frames.is_empty() {
                            if self.current_co != 0 {
                                let caller = self.coroutines[self.current_co].caller.unwrap_or(0);
                                self.coroutines[self.current_co].state = CoroutineState::Dead;
                                self.swap_context(caller);
                                break;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    GET_RET => {
                        let target = (instruction >> 16) as u8;
                        let index = (instruction >> 8) as u8;

                        let var = self.return_values.get(index as usize).copied().unwrap_or(Variable::Nil);
                        self.set_register(target as usize, var);
                    }

                    // endregion:functions
                    // region:objects
                    NEW_ARRAY => {
                        let target = (instruction >> 16) as u8;
                        
                        // Allocate array and set the target to the pointer
                        let array_idx = self.arrays.insert(Array::new());
                        self.set_register(target as usize, Variable::Array(array_idx));
                    }
                    NEW_TABLE => {
                        let target = (instruction >> 16) as u8;
                        
                        // Allocate array and set the target to the pointer
                        let table_idx = self.tables.insert(Table::new());
                        self.set_register(target as usize, Variable::Table(table_idx));
                    }
                    GET_FIELD => {
                        let dest = (instruction >> 16) as u8;
                        let arr_reg = (instruction >> 8) as u8;
                        let idx_reg = instruction as u8;

                        let indexing = match self.functions.get(current_func_idx) {
                            Some(c) => c.array_indexing,
                            _ => unreachable!(),
                        };

                        match self.get_register(arr_reg) {
                            Variable::Array(array_idx) => {
                                let array = match self.arrays.get(array_idx) {
                                    Some(a) => a,
                                    None => {
                                        return Err(RuntimeError::InternalError(
                                            format!("Invalid array handle {array_idx}")
                                        ))
                                    }
                                };

                                match self.get_register(idx_reg) {
                                    Variable::Float(f) => {
                                        self.set_register(dest as usize, array.get((f - indexing as f64) as usize));
                                    }
                                    var => {
                                        return Err(RuntimeError::TypeError(format!(
                                            "attempted to index array with {}",
                                            var.type_name()
                                        )));
                                    }
                                }
                            }
                            Variable::Table(table_idx) => {
                                let table = match self.tables.get(table_idx) {
                                    Some(a) => a,
                                    None => {
                                        return Err(RuntimeError::InternalError(
                                            format!("Invalid table handle {table_idx}")
                                        ))
                                    }
                                };

                                match self.get_register(idx_reg) {
                                    Variable::Float(f) => {
                                        self.set_register(dest as usize, table.get(Variable::Float(f - indexing as f64)));
                                    }
                                    var => self.set_register(dest as usize, table.get(var)),
                                }
                            }
                            var => {
                                return Err(RuntimeError::TypeError(format!(
                                    "attempted to index {}",
                                    var.type_name()
                                )));
                            }
                        }
                    }
                    SET_FIELD => {
                        let target = (instruction >> 16) as u8;
                        let index = (instruction >> 8) as u8;
                        let val = instruction as u8;

                        let target_var = self.get_register(target);
                        let idx_var = self.get_register(index);
                        let value_to_set = self.get_register(val);

                        let indexing = match self.functions.get(current_func_idx) {
                            Some(c) => c.array_indexing,
                            _ => unreachable!(),
                        };

                        // Read and validate the target register
                        match target_var {
                            Variable::Array(array_idx) => {
                                // Read and validate the index register
                                let idx = match idx_var {
                                    Variable::Float(f) => (f - indexing as f64) as usize,
                                    var => {
                                        return Err(RuntimeError::TypeError(format!(
                                            "attempted to index array with {}",
                                            var.type_name()
                                        )));
                                    }
                                };

                                // Read the value to insert
                                let array = match self.arrays.get_mut(array_idx) {
                                    Some(a) => a,
                                    None => {
                                        return Err(RuntimeError::InternalError(format!(
                                            "Invalid array handle {array_idx}"
                                        )));
                                    }
                                };

                                array.set(idx, value_to_set);
                            }
                            Variable::Table(table_idx) => {
                                let idx = match idx_var {
                                    Variable::Float(f) => Variable::Float(f - indexing as f64),
                                    var => var
                                };

                                let table = match self.tables.get_mut(table_idx) {
                                    Some(a) => a,
                                    None => {
                                        return Err(RuntimeError::InternalError(format!(
                                            "Invalid table handle {table_idx}"
                                        )));
                                    }
                                };

                                table.set(idx, value_to_set);
                            }
                            var => {
                                return Err(RuntimeError::TypeError(format!(
                                    "attempted to index {}",
                                    var.type_name()
                                )));
                            }
                        };
                    }
                    APPEND_ARRAY => {
                        let target = (instruction >> 16) as u8;
                        let val = (instruction >> 8) as u8;

                        let target_var = self.get_register(target);
                        let value_to_set = self.get_register(val);

                        // Match on the target variable
                        match target_var {
                            Variable::Array(array_idx) => {
                                let array = match self.arrays.get_mut(array_idx) {
                                    Some(a) => a,
                                    None => {
                                        return Err(RuntimeError::InternalError(format!(
                                            "invalid array handle {array_idx}"
                                        )));
                                    }
                                };

                                array.set(array.len(), value_to_set);
                            }
                            Variable::Table(table_idx) => {
                                let table = match self.tables.get_mut(table_idx) {
                                    Some(t) => t,
                                    None => {
                                        return Err(RuntimeError::InternalError(format!(
                                            "invalid table handle {table_idx}"
                                        )));
                                    }
                                };

                                table.set(Variable::Float(table.array.len() as f64), value_to_set);
                            }
                            var => {
                                return Err(RuntimeError::TypeError(format!(
                                    "attempted to append to {}",
                                    var.type_name()
                                )));
                            }
                        }
                    }
                    // endregion:arrays

                    // This is intended for debugging
                    // It cannot catch opcodes that are undefined, only unimplemented ones.
                    #[allow(unreachable_patterns)]
                    unimplemented_op => return Err(RuntimeError::NotFound(format!("Could not execute opcode `{}`, as no functionality is defined for it.", bytecode::opcode_name(unimplemented_op as u8))))
                }
            } 
        }
        Ok(())
    }

    /// Calls a function and passes the given arguments.
    /// 
    /// Returns `true` if the VM context was modified (e.g., a new frame was pushed or a coroutine swapped)
    /// which signals that the execution loop needs to yield/restart its context.
    pub fn call(&mut self, func: Variable, args: &[Variable]) -> Result<bool, RuntimeError> {
        // Validate that the variable is actually a function
        let func_idx = match func {
            Variable::Function(idx) => idx,
            _ => return Err(RuntimeError::TypeError(
                format!("attempt to call a {} value", func.type_name())
            )),
        };

        let rust_func = match self.functions.get(func_idx) {
            Some(c) => c.rust_function,
            None => {
                return Err(RuntimeError::InternalError(
                    format!("Invalid function handle {func_idx}")
                ))
            }
        };

        let prev_co = self.current_co;
        let prev_frames = self.frames.len();

        if let Some(func) = rust_func {
            self.call_stack.clear();
            self.call_stack.extend_from_slice(args);
            
            // Execute the Rust function directly
            self.native_depth += 1;
            let result = func(self);
            self.native_depth -= 1;
            result?;
            
            // Returns true if the rust function triggered a coroutine swap or pushed a bytecode frame
            Ok(self.current_co != prev_co || self.frames.len() != prev_frames)
        } else {
            let new_base = self.registers.len();
            self.registers.extend_from_slice(args);
            
            self.frames.push(CallFrame {
                func_idx,
                ip: 0,
                register_base: new_base,
                from_rust: true,
            });

            self.run()?;
            
            // Always returns true because lua definitely pushed a new frame onto the stack
            Ok(true) 
        }
    }

    // endregion:exec

    // UNUSED: this is for later
    pub fn collect_garbage(&mut self) {
        use std::collections::HashSet;

        let mut marked_arrays = HashSet::new();
        let mut marked_strings = HashSet::new();

        // Mark, find all reachable variables
        let mut worklist: Vec<Variable> = Vec::new();

        // Collect roots from registers, globals, and stack
        worklist.extend(self.registers.iter().copied());
        worklist.extend(self.globals.values().copied());
        worklist.extend(self.call_stack.iter().copied());

        // Not sure if this needs to run on return values, but im concerned about a situation like:
        // `var["a" .. "b"] = func()` where it might call the gc between RET and GET_RET.
        // Either way, this is basically free compared to the rest of the gc so preventing edge cases that may or may not exist is a better idea i think.
        worklist.extend(self.return_values.iter().copied());

        // Traverse the worklist
        while let Some(var) = worklist.pop() {
            match var {
                Variable::Array(idx) => {
                    if marked_arrays.insert(idx) {
                        // If it hasn't seen this array yet, trace its elements too
                        if let Some(arr) = self.arrays.get(idx) {
                            worklist.extend(arr.0.iter().copied());
                        }
                    }
                }
                Variable::String(idx) => {
                    marked_strings.insert(idx);
                }
                _ => {}
            }
        }

        // Sweep, remove unreferenced slots from the Slabs
        self.arrays.retain(|idx, _| marked_arrays.contains(&idx));

        self.interned_strings.retain(|_, idx| marked_strings.contains(idx));
        self.strings.retain(|idx, _| marked_strings.contains(&idx));
    }
}