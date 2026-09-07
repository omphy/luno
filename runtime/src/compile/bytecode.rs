use crate::utils::set_index;
use crate::rust_functions::NativeFn;
#[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))]
use crate::vm::VmContext;

/// Opcodes tell the VM what to do in an instruction. The opcodes are simply to give a name to a u8 and don't actually contain any information like the arguments. 
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum Opcode {
    // Load
    /// Loads a string (target: u8, pos: u16)
    LOADSTR,

    /// Loads a bool (i need to finish these)
    LOADBOOL,
    LOADFUNC,
    LOADNIL,
    LOADFLOAT,
    
    // Math
    ADD,
    MIN,
    MUL,
    DIV,
    POW,
    CONCAT,

    // General opcode
    MOV,

    // Branching opcode
    LT,
    LTEQ,
    EQ,
    NOTEQ,
    SKIP_ON_TRUE,
    JMP,

    // Closure opcode
    // GET_UPVAL,
    // SET_UPVAL,

    // Function opcode
    CALL,
    RET,
    GET_RET,

    // Globals
    SET_GLOBAL,
    GET_GLOBAL,

    // Arrays, tables and hashes
    NEW_ARRAY,
    NEW_TABLE,
    GET_FIELD,
    SET_FIELD,
    APPEND_ARRAY,
}

/// Return a readable name for an opcode byte.
pub fn opcode_name(op: u8) -> String {
    let name = match op {
        x if x == Opcode::LOADSTR as u8 => "LOADSTR",
        x if x == Opcode::LOADBOOL as u8 => "LOADBOOL",
        x if x == Opcode::LOADFUNC as u8 => "LOADFUNC",
        x if x == Opcode::LOADNIL as u8 => "LOADNIL",
        x if x == Opcode::LOADFLOAT as u8 => "LOADFLOAT",

        x if x == Opcode::ADD as u8 => "ADD",
        x if x == Opcode::MIN as u8 => "MIN",
        x if x == Opcode::MUL as u8 => "MUL",
        x if x == Opcode::DIV as u8 => "DIV",
        x if x == Opcode::POW as u8 => "POW",
        x if x == Opcode::CONCAT as u8 => "CONCAT",

        x if x == Opcode::MOV as u8 => "MOV",

        x if x == Opcode::LT as u8 => "LT",
        x if x == Opcode::LTEQ as u8 => "LTEQ",
        x if x == Opcode::EQ as u8 => "EQ",
        x if x == Opcode::NOTEQ as u8 => "NOTEQ",
        x if x == Opcode::SKIP_ON_TRUE as u8 => "SKIP_ON_TRUE",

        x if x == Opcode::JMP as u8 => "JMP",

        // x if x == Opcode::GET_UPVAL as u8 => "GET_UPVAL",
        // x if x == Opcode::SET_UPVAL as u8 => "SET_UPVAL",

        x if x == Opcode::GET_GLOBAL as u8 => "GET_GLOBAL",
        x if x == Opcode::SET_GLOBAL as u8 => "SET_GLOBAL",

        x if x == Opcode::CALL as u8 => "CALL",
        x if x == Opcode::RET as u8 => "RET",
        x if x == Opcode::GET_RET as u8 => "GET_RET",

        x if x == Opcode::NEW_ARRAY as u8 => "NEW_ARRAY",
        x if x == Opcode::NEW_TABLE as u8 => "NEW_TABLE",
        x if x == Opcode::GET_FIELD as u8 => "GET_FIELD",
        x if x == Opcode::SET_FIELD as u8 => "SET_FIELD",
        x if x == Opcode::APPEND_ARRAY as u8 => "APPEND_ARRAY",

        // Early return for the fallback so we don't allocate unless we have to
        _ => return format!("0x{:02X}", op),
    };

    name.to_string()
}

#[inline(always)]
pub const fn pack_u32_4x8(b1: u8, b2: u8, b3: u8, b4: u8) -> u32 {
    ((b1 as u32) << 24) | ((b2 as u32) << 16) | ((b3 as u32) << 8) | (b4 as u32)
}

#[inline(always)]
pub const fn pack_u32_2x8_1x16(b1: u8, b2: u8, b3: u16) -> u32 {
    ((b1 as u32) << 24) | ((b2 as u32) << 16) | (b3 as u32)
}

/// A chunk is a block that contains code and other info. It is essentially a function.
#[derive(Clone, Debug)]
pub struct Chunk {
    pub code: Vec<u32>,
    pub constants: Vec<u64>,
    pub string_constants: Vec<String>,
    pub functions: Vec<Chunk>,
    pub rust_function: Option<NativeFn>,
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))]
    pub jit_compiled: Option<unsafe extern "C" fn(*mut VmContext)>,
    pub num_registers: usize,
    pub array_indexing: u8,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            string_constants: Vec::new(),
            functions: Vec::new(),
            rust_function: None,
            #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64", feature = "jit"))]
            jit_compiled: None,
            num_registers: 0,
            array_indexing: 0,
        }
    }

    /// Internal helper to update the number of registers required by this chunk [1].
    #[inline(always)]
    pub fn track_register(&mut self, reg: u8) {
        let needed = (reg as usize) + 1;
        if needed > self.num_registers {
            self.num_registers = needed;
        }
    }

    pub fn disassemble(&self, name: &str) {
        println!("\n{}", "=".repeat(70));
        println!(" CHUNK: {:<58}", name);
        println!("{}", "=".repeat(70));
        println!(
            " Registers Used : {}\n Instructions   : {}\n Constants      : {}\n Strings        : {}\n Functions      : {}",
            self.num_registers,
            self.code.len(),
            self.constants.len(),
            self.string_constants.len(),
            self.functions.len()
        );

        // 1. Print Constants Pool (Numbers)
        if !self.constants.is_empty() {
            println!("\n [Constants Pool]");
            for (i, &raw_bits) in self.constants.iter().enumerate() {
                let float_val = f64::from_bits(raw_bits);
                println!("   [{:>3}] {:<18} (bits: 0x{:016X})", i, float_val, raw_bits);
            }
        }

        // 2. Print String Pool
        if !self.string_constants.is_empty() {
            println!("\n [String Pool]");
            for (i, s) in self.string_constants.iter().enumerate() {
                println!("   [{:>3}] {:?}", i, s);
            }
        }

        // 3. Print Nested Functions Pool Summary
        if !self.functions.is_empty() {
            println!("\n [Nested Functions Pool]");
            for (i, func_chunk) in self.functions.iter().enumerate() {
                println!(
                    "   [{:>3}] <Chunk: {}_func_{} | {} instrs | {} regs>",
                    i,
                    name,
                    i,
                    func_chunk.code.len(),
                    func_chunk.num_registers
                );
            }
        }

        // 4. Print Instruction Disassembly Table
        println!("\n {}", "-".repeat(68));
        println!("  IP   | RAW HEX    | OPCODE          | OPERANDS / DETAILS");
        println!(" {}", "-".repeat(68));

        for (i, &instruction) in self.code.iter().enumerate() {
            let opcode = ((instruction >> 24) & 0xFF) as u8;
            let b2 = ((instruction >> 16) & 0xFF) as u8;
            let b3 = ((instruction >> 8) & 0xFF) as u8;
            let b4 = (instruction & 0xFF) as u8;
            let u16_val = (instruction & 0xFFFF) as u16;

            let op_name = opcode_name(opcode);

            let operands_display = match opcode {
                // Constants & Literals
                x if x == Opcode::LOADFLOAT as u8 => {
                    let val_str = self
                        .constants
                        .get(u16_val as usize)
                        .map(|&c| format!("{}", f64::from_bits(c)))
                        .unwrap_or_else(|| "<out of bounds>".to_string());
                    format!("r{}, const[{}] ({})", b2, u16_val, val_str)
                }
                x if x == Opcode::LOADSTR as u8 => {
                    let str_val = self
                        .string_constants
                        .get(u16_val as usize)
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_else(|| "<out of bounds>".to_string());
                    format!("r{}, str[{}] ({})", b2, u16_val, str_val)
                }
                x if x == Opcode::LOADBOOL as u8 => {
                    format!("r{}, {}", b2, b3 != 0)
                }
                x if x == Opcode::LOADNIL as u8 => {
                    format!("r{}", b2)
                }
                x if x == Opcode::LOADFUNC as u8 => {
                    format!("r{}, func[{}]", b2, u16_val)
                }

                // Math & Logic (r_dest, r_left, r_right)
                x if x == Opcode::ADD as u8
                    || x == Opcode::MIN as u8
                    || x == Opcode::MUL as u8
                    || x == Opcode::DIV as u8
                    || x == Opcode::POW as u8
                    || x == Opcode::CONCAT as u8
                    || x == Opcode::EQ as u8
                    || x == Opcode::NOTEQ as u8
                    || x == Opcode::LT as u8
                    || x == Opcode::LTEQ as u8 =>
                {
                    format!("r{}, r{}, r{}", b2, b3, b4)
                }

                // General
                x if x == Opcode::MOV as u8 => {
                    format!("r{}, r{}", b2, b3)
                }

                // Control Flow & Branching
                x if x == Opcode::SKIP_ON_TRUE as u8 => {
                    format!("condition: r{} (skips next instr if truthy)", b2)
                }
                x if x == Opcode::JMP as u8 => {
                    // Sign extend 24-bit jump offset
                    let raw_24 = (instruction & 0x00FF_FFFF) as i32;
                    let offset = if (raw_24 & 0x0080_0000) != 0 {
                        raw_24 | !0x00FF_FFFF
                    } else {
                        raw_24
                    };
                    let target_ip = (i as i32) + offset;
                    format!("offset {:+} (target: {:04})", offset, target_ip)
                }

                // Arrays & Fields
                x if x == Opcode::NEW_ARRAY as u8 => {
                    format!("r{}", b2)
                }
                x if x == Opcode::NEW_TABLE as u8 => {
                    format!("r{}", b2)
                }
                x if x == Opcode::APPEND_ARRAY as u8 => {
                    format!("array: r{}, val: r{}", b2, b3)
                }
                x if x == Opcode::GET_FIELD as u8 => {
                    format!("target: r{}, index: r{}", b2, b3)
                }
                x if x == Opcode::SET_FIELD as u8 => {
                    format!("array: r{}, index: r{}, val: r{}", b2, b3, b4)
                }

                // Functions / Calls / Returns
                x if x == Opcode::CALL as u8 => {
                    if b4 == 0 {
                        format!("fn: r{}, (no args)", b2)
                    } else if b4 == 1 {
                        format!("fn: r{}, arg: r{}", b2, b3)
                    } else {
                        format!("fn: r{}, args: r{}..=r{}", b2, b3, b3 + b4 - 1)
                    }
                }
                x if x == Opcode::RET as u8 => {
                    if b4 == 0 {
                        "void".to_string()
                    } else if b4 == 1 {
                        format!("r{}", b3)
                    } else {
                        format!("r{}..=r{}", b3, b3 + b4 - 1)
                    }
                }
                x if x == Opcode::GET_RET as u8 => {
                    format!("target: r{}, ret_idx: {}", b2, b3)
                }

                // Globals
                x if x == Opcode::SET_GLOBAL as u8 => {
                    let str_val = self
                        .string_constants
                        .get(u16_val as usize)
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_else(|| "<out of bounds>".to_string());
                    format!("globals[{}] ({}) = r{}", u16_val, str_val, b2)
                }
                x if x == Opcode::GET_GLOBAL as u8 => {
                    let str_val = self
                        .string_constants
                        .get(u16_val as usize)
                        .map(|s| format!("{:?}", s))
                        .unwrap_or_else(|| "<out of bounds>".to_string());
                    format!("r{} = globals[{}] ({})", b2, u16_val, str_val)
                }

                _ => format!("b2: {:02X}, b3: {:02X}, b4: {:02X}", b2, b3, b4),
            };

            println!(
                " {:04}  | 0x{:08X} | {:<15} | {}",
                i, instruction, op_name, operands_display
            );
        }
        println!(" {}\n", "-".repeat(68));

        // 5. Recursively disassemble nested functions
        for (i, func_chunk) in self.functions.iter().enumerate() {
            func_chunk.disassemble(&format!("{}_func_{}", name, i));
        }
    }

    pub fn add_loadfloat(&mut self, target: u8, value: f64) {
        self.track_register(target); // <-- Track target register
        if let Some(index) = self.resolve_constant(value.to_bits()) {
            self.code
                .push(pack_u32_2x8_1x16(Opcode::LOADFLOAT as u8, target, index));
        } else {
            let pos = self.constants.len() as u16;
            set_index(&mut self.constants, pos as usize, value.to_bits());
            self.code
                .push(pack_u32_2x8_1x16(Opcode::LOADFLOAT as u8, target, pos));
        }
    }

    pub fn add_loadbool(&mut self, target: u8, value: bool) {
        self.track_register(target); // <-- Track target register
        self.code.push(pack_u32_4x8(
            Opcode::LOADBOOL as u8,
            target,
            value as u8,
            0,
        ));
    }

    pub fn add_loadnil(&mut self, target: u8) {
        self.track_register(target); // <-- Track target register
        self.code
            .push(pack_u32_4x8(Opcode::LOADNIL as u8, target, 0, 0));
    }

    pub fn add_loadfunc(&mut self, target: u8, func: Chunk) {
        self.track_register(target); // <-- Track target register
        let func_pos = self.functions.len() as u16;
        self.functions.push(func);

        self.code
            .push(pack_u32_2x8_1x16(Opcode::LOADFUNC as u8, target, func_pos));
    }

    pub fn add_loadstr(&mut self, target: u8, value: &str) {
        self.track_register(target); // <-- Track target register
        let string_pos = self.string_constants.len() as u16;
        self.string_constants.push(value.to_string());

        self.code.push(pack_u32_2x8_1x16(
            Opcode::LOADSTR as u8,
            target,
            string_pos,
        ));
    }

    pub fn add_skip_on_true(&mut self, condition: u8) {
        self.track_register(condition); // <-- Track condition register
        self.code
            .push(pack_u32_4x8(Opcode::SKIP_ON_TRUE as u8, condition, 0, 0));
    }

    pub fn add_jump(&mut self, offset: i32) {
        self.code.push(pack_u32_2x8_1x16(
            Opcode::JMP as u8,
            (offset >> 16) as u8,
            offset as u16,
        ));
    }

    pub fn add_newarray(&mut self, target: u8) {
        self.track_register(target); // <-- Track target register
        self.code
            .push(pack_u32_4x8(Opcode::NEW_ARRAY as u8, target, 0, 0));
    }

    pub fn add_newtable(&mut self, target: u8) {
        self.track_register(target); // <-- Track target register
        self.code
            .push(pack_u32_4x8(Opcode::NEW_TABLE as u8, target, 0, 0));
    }

    pub fn add_append_array(&mut self, target: u8, value: u8) {
        self.track_register(target);
        self.code
            .push(pack_u32_4x8(Opcode::APPEND_ARRAY as u8, target, value, 0));
    }

    pub fn add_get_field(&mut self, target: u8, object: u8, index: u8) {
        self.track_register(target);
        self.code
            .push(pack_u32_4x8(Opcode::GET_FIELD as u8, target, object, index));
    }

    pub fn add_set_field(&mut self, target: u8, index: u8, val: u8) {
        self.track_register(target);
        self.code
            .push(pack_u32_4x8(Opcode::SET_FIELD as u8, target, index, val));
    }

    // Math
    pub fn add_add(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::ADD as u8, target, left, right));
    }

    pub fn add_min(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::MIN as u8, target, left, right));
    }

    pub fn add_mul(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::MUL as u8, target, left, right));
    }

    pub fn add_div(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::DIV as u8, target, left, right));
    }

    pub fn add_pow(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::POW as u8, target, left, right));
    }

    pub fn add_concat(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::CONCAT as u8, target, left, right));
    }

    pub fn add_eq(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::EQ as u8, target, left, right));
    }

    pub fn add_noteq(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code.push(pack_u32_4x8(
            Opcode::NOTEQ as u8,
            target,
            left,
            right,
        ));
    }

    pub fn add_lt(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code
            .push(pack_u32_4x8(Opcode::LT as u8, target, left, right));
    }

    pub fn add_lteq(&mut self, target: u8, left: u8, right: u8) {
        self.track_register(target);
        self.track_register(left);
        self.track_register(right);
        self.code.push(pack_u32_4x8(
            Opcode::LTEQ as u8,
            target,
            left,
            right,
        ));
    }

    pub fn add_mov(&mut self, target: u8, register: u8) {
        self.track_register(target);
        self.track_register(register);
        self.code
            .push(pack_u32_4x8(Opcode::MOV as u8, target, register, 0));
    }

    pub fn add_call(&mut self, target: u8, start: u8, offset: u8) {
        self.track_register(target);
        for i in 0..offset {
            self.track_register(start + i);
        }
        self.code
            .push(pack_u32_4x8(Opcode::CALL as u8, target, start, offset));
    }

    pub fn add_ret(&mut self, start: u8, offset: u8) {
        for i in 0..offset {
            self.track_register(start + i);
        }
        self.code
            .push(pack_u32_4x8(Opcode::RET as u8, 0, start, offset));
    }

    pub fn add_get_ret(&mut self, target: u8, index: u8) {
        self.track_register(target);
        self.code
            .push(pack_u32_4x8(Opcode::GET_RET as u8, target, index, 0));
    }

    pub fn add_set_global(&mut self, name: &str, value: u8) {
        self.track_register(value);
        let string_pos = self.string_constants.len() as u16;
        self.string_constants.push(name.to_string());

        self.code
            .push(pack_u32_2x8_1x16(Opcode::SET_GLOBAL as u8, value, string_pos));
    }

    pub fn add_get_global(&mut self, name: &str, target: u8) {
        self.track_register(target);
        let string_pos = self.string_constants.len() as u16;
        self.string_constants.push(name.to_string());

        self.code
            .push(pack_u32_2x8_1x16(Opcode::GET_GLOBAL as u8, target, string_pos));
    }

    fn resolve_constant(&self, value: u64) -> Option<u16> {
        self.constants
            .iter()
            .position(|&c| c == value)
            .map(|i| i as u16)
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}