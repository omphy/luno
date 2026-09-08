// TODO: rewrite from the ground up, its outdated, non-functional, weird, and a bunch of pure ai generated slopcode

use cranelift::{
    module::{
        default_libcall_names,
        Linkage, Module
    },
    codegen::{
        Context,
        ir::{Block, BlockArg}
    },
    jit::{JITBuilder, JITModule},
    frontend::{Switch, Variable},
    prelude::*
};
use crate::{
    vm::{
        HeapObject, HeapValue,
        Types, VmContext
    },
    memory::*,
    helper::*,
};

use crate::bytecode::*;

pub struct JIT {
    module: JITModule,
    context: Context,
    builder_context: FunctionBuilderContext,
    helpers: HelperIds,
}

#[unsafe(no_mangle)]
pub extern "C" fn vm_load_string(
    ctx_ptr: *mut VmContext,
    str_ptr: *const u8,
    str_len: usize,
    target: u8,
) {
    let ctx = unsafe { &mut *ctx_ptr };
    let vm = unsafe { &mut *ctx.vm };

    let slice = unsafe { std::slice::from_raw_parts(str_ptr, str_len) };
    let string_val = unsafe { std::str::from_utf8_unchecked(slice) }.to_string();

    vm.try_run_gc();

    let object = Some(HeapObject {
        value: HeapValue::String(string_val),
        marked: false,
    });

    let position = match vm.get_free_slot() {
        Some(pos) => {
            vm.heap[pos] = object;
            pos
        }
        None => {
            let pos = vm.heap.len();
            vm.heap.push(object);
            pos
        }
    };

    let abs_idx = ctx.frame_pointer + target as usize;
    unsafe {
        *ctx.reg_values.add(abs_idx) = position as u64;
        *ctx.reg_types.add(abs_idx) = Types::STRING as u8;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vm_get_global(
    ctx_ptr: *mut VmContext, 
    name_ptr: *const u8, 
    name_len: usize, 
    target: u8
) {
    let ctx = unsafe { &mut *ctx_ptr };
    let vm = unsafe { &*ctx.vm }; 
    
    let name_slice = unsafe { std::slice::from_raw_parts(name_ptr, name_len) };
    let name = unsafe { std::str::from_utf8_unchecked(name_slice) };

    let (value, vtype) = match vm.globals.get(name) {
        Some(global) => (global.value, global.vtype),
        None => (0, Types::NIL as u8),
    };

    let abs_idx = ctx.frame_pointer + target as usize;
    unsafe {
        *ctx.reg_values.add(abs_idx) = value;
        *ctx.reg_types.add(abs_idx) = vtype;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vm_call(
    ctx_ptr: *mut VmContext,
    target: u8,
    start: u8,
    arg_count: u8,
) {
    let ctx = unsafe { &mut *ctx_ptr };
    let vm = unsafe { &mut *ctx.vm }; 
    vm.call(target, start, arg_count);
}

impl JIT {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").expect("Failed to set opt level");
        
        let isa_builder = cranelift::native::builder().unwrap();
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();

        let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
        
        builder.symbol("vm_get_global", vm_get_global as *const u8);
        builder.symbol("vm_call", vm_call as *const u8);
        builder.symbol("vm_load_string", vm_load_string as *const u8);

        let mut module = JITModule::new(builder);
        let helpers = HelperIds::new(&mut module);

        Self {
            context: module.make_context(),
            builder_context: FunctionBuilderContext::new(),
            module,
            helpers,
        }
    }

    pub fn compile_chunk(&mut self, chunk: &Chunk) -> unsafe extern "C" fn(*mut VmContext) {
        self.context.func.signature.params.push(AbiParam::new(types::I64)); 

        {
            let mut bcx = FunctionBuilder::new(&mut self.context.func, &mut self.builder_context);
            let entry_block = bcx.create_block();
            bcx.append_block_param(entry_block, types::I64); 
            bcx.switch_to_block(entry_block);

            let ctx_ptr = bcx.block_params(entry_block)[0];
            let helper_refs = HelperRefs::new(&self.helpers, &mut self.module, &mut bcx.func);
            let len = chunk.code.len();

            let num_regs = chunk.num_registers as usize;
            let mut val_vars = vec![];
            let mut type_vars = vec![];

            for i in 0..num_regs {
                let v_val = bcx.declare_var(types::I64);
                let v_type = bcx.declare_var(types::I8);
                val_vars.push(v_val);
                type_vars.push(v_type);

                let val = load_memory_value(&mut bcx, ctx_ptr, i as u8);
                let vtype = load_memory_type(&mut bcx, ctx_ptr, i as u8);
                bcx.def_var(v_val, val);
                bcx.def_var(v_type, vtype);
            }

            let mut blocks: Vec<Option<Block>> = vec![None; len + 1];

            for i in 0..len {
                let instruction = chunk.code[i];
                let opcode = (instruction >> 24) as u8;

                match opcode {
                    JMP => {
                        let offset = ((instruction << 8) as i32) >> 8;
                        let target_idx = (i as i32 + offset) as usize;
                        if target_idx > len { panic!("JMP out of bounds!"); }
                        if blocks[target_idx].is_none() { blocks[target_idx] = Some(bcx.create_block()); }
                    }
                    SKIP_ON_TRUE => {
                        let skip_idx = i + 2;      
                        let fallthrough_idx = i + 1; 
                        if skip_idx > len || fallthrough_idx > len { panic!("SKIP_ON_TRUE out of bounds!"); }
                        if blocks[skip_idx].is_none() { blocks[skip_idx] = Some(bcx.create_block()); }
                        if blocks[fallthrough_idx].is_none() { blocks[fallthrough_idx] = Some(bcx.create_block()); }
                    }
                    _ => continue,
                }
            }

            let mut i = 0;
            let mut terminated = false; 

            while i < len {
                if let Some(target_block) = blocks[i] {
                    if !terminated {
                        bcx.ins().jump(target_block, &[]);
                    }
                    bcx.switch_to_block(target_block);
                    terminated = false; 
                }

                compile_instruction(
                    &mut bcx, 
                    chunk.code[i], 
                    chunk, 
                    ctx_ptr,
                    &helper_refs,
                    &blocks,         
                    i,               
                    &mut terminated, 
                    &val_vars,  
                    &type_vars, 
                    num_regs // Passed num_regs here instead of used_regs
                );
                i += 1;
            }

            if let Some(end_block) = blocks[len] {
                if !terminated {
                    bcx.ins().jump(end_block, &[]);
                }
                bcx.switch_to_block(end_block);
                flush_registers(&mut bcx, ctx_ptr, &val_vars, &type_vars, num_regs);
                bcx.ins().return_(&[]);
            } 
            else if !terminated {
                flush_registers(&mut bcx, ctx_ptr, &val_vars, &type_vars, num_regs);
                bcx.ins().return_(&[]);
            }
            
            bcx.seal_all_blocks();
            bcx.finalize();
        }

        let func_id = self.module
            .declare_function("add_ones", Linkage::Export, &self.context.func.signature)
            .expect("Failed to declare function");

        self.module.define_function(func_id, &mut self.context).expect("Failed to compile function");
        self.module.clear_context(&mut self.context);
        self.module.finalize_definitions().expect("Failed to finalize JIT module");

        let code_ptr = self.module.get_finalized_function(func_id);
        let callable_fn: unsafe extern "C" fn(*mut VmContext) = unsafe { std::mem::transmute(code_ptr) };

        callable_fn
    }
}

fn read_register_type(bcx: &mut FunctionBuilder, type_vars: &[Variable], target: u8) -> Value {
    bcx.use_var(type_vars[target as usize])
}

fn read_register_value(bcx: &mut FunctionBuilder, val_vars: &[Variable], target: u8) -> Value {
    bcx.use_var(val_vars[target as usize])
}

fn write_register_dynamic(bcx: &mut FunctionBuilder, val_vars: &[Variable], type_vars: &[Variable], target: u8, val: Value, vtype: Value) {
    bcx.def_var(val_vars[target as usize], val);
    bcx.def_var(type_vars[target as usize], vtype);
}

fn write_register(bcx: &mut FunctionBuilder, val_vars: &[Variable], type_vars: &[Variable], target: u8, val: Value, vtype: u8) {
    let type_val = bcx.ins().iconst(types::I8, vtype as i64);
    write_register_dynamic(bcx, val_vars, type_vars, target, val, type_val);
}

fn check_equal(bcx: &mut FunctionBuilder, type_left: Value, type_right: Value, value_left: Value, value_right: Value) -> Value {
    let block_dispatch = bcx.create_block();
    let block_int_compare = bcx.create_block();
    let block_float_compare = bcx.create_block();
    let block_true = bcx.create_block();
    let block_false = bcx.create_block();
    let block_end = bcx.create_block();

    bcx.append_block_param(block_end, types::I64);

    let false_val = bcx.ins().iconst(types::I64, 0);
    let true_val = bcx.ins().iconst(types::I64, 1);

    let types_match = bcx.ins().icmp(IntCC::Equal, type_left, type_right);
    bcx.ins().brif(types_match, block_dispatch, &[], block_false, &[]); 
    
    bcx.switch_to_block(block_dispatch);
    let mut switch = Switch::new();
    switch.set_entry(Types::STRING as u128, block_int_compare);
    switch.set_entry(Types::F64 as u128, block_float_compare);  
    switch.set_entry(Types::BOOL as u128, block_int_compare);
    switch.set_entry(Types::I64 as u128, block_int_compare);
    switch.set_entry(Types::NIL as u128, block_true);

    switch.emit(bcx, type_left, block_false);

    bcx.switch_to_block(block_int_compare);
    let int_eq = bcx.ins().icmp(IntCC::NotEqual, value_left, value_right);
    let int_eq_i64 = bcx.ins().uextend(types::I64, int_eq);
    bcx.ins().jump(block_end, &[BlockArg::Value(int_eq_i64)]);
    
    bcx.switch_to_block(block_float_compare);
    let f_left = bcx.ins().bitcast(types::F64, MemFlagsData::new(), value_left);
    let f_right = bcx.ins().bitcast(types::F64, MemFlagsData::new(), value_right);
    let float_eq = bcx.ins().fcmp(FloatCC::Equal, f_left, f_right);
    let float_eq_i64 = bcx.ins().uextend(types::I64, float_eq);
    bcx.ins().jump(block_end, &[BlockArg::Value(float_eq_i64)]);
    
    bcx.switch_to_block(block_true);
    bcx.ins().jump(block_end, &[BlockArg::Value(true_val)]);

    bcx.switch_to_block(block_false);
    bcx.ins().jump(block_end, &[BlockArg::Value(false_val)]);

    bcx.switch_to_block(block_end);
    bcx.block_params(block_end)[0]
}

fn compile_instruction(
    bcx: &mut FunctionBuilder,
    instruction: u32,
    chunk: &Chunk,
    ctx_ptr: Value,
    helpers: &HelperRefs,
    blocks: &[Option<Block>],
    i: usize,
    terminated: &mut bool,
    val_vars: &[Variable],
    type_vars: &[Variable],
    num_regs: usize, // <--- Used to be `used_regs`
) {
    let opcode = (instruction >> 24) as u8;
    match opcode {
        LOADF64 => {
            let target = (instruction >> 16) as u8;
            let arg = instruction as u16;
            
            let val_bits = chunk.constants[arg as usize] as i64;
            let val_i64 = bcx.ins().iconst(types::I64, val_bits);
            
            write_register(bcx, val_vars, type_vars, target, val_i64, Types::F64 as u8);
        }
        ADDF64 => {
            let target = (instruction >> 16) as u8;
            let left = (instruction >> 8) as u8;
            let right = instruction as u8;

            let left_val = read_register_value(bcx, val_vars, left);
            let right_val = read_register_value(bcx, val_vars, right);

            let f_left = bcx.ins().bitcast(types::F64, MemFlagsData::new(), left_val);
            let f_right = bcx.ins().bitcast(types::F64, MemFlagsData::new(), right_val);

            let result = bcx.ins().fadd(f_left, f_right);
            
            let result_i64 = bcx.ins().bitcast(types::I64, MemFlagsData::new(), result);
            
            write_register(bcx, val_vars, type_vars, target, result_i64, Types::F64 as u8);
        }
        LOADBOOL => {
            let target = (instruction >> 16) as u8;
            let arg = (instruction >> 8) as u8;
            
            let bool_val = bcx.ins().iconst(types::I64, arg as i64);
            
            write_register(bcx, val_vars, type_vars, target, bool_val, Types::BOOL as u8);
        }
        LOADSTR => {
            let target = (instruction >> 16) as u8;
            let arg = instruction as u16;
            let string = &chunk.string_constants[arg as usize];

            let str_ptr_val = bcx.ins().iconst(types::I64, string.as_ptr() as i64);
            let str_len_val = bcx.ins().iconst(types::I64, string.len() as i64);
            let target_val = bcx.ins().iconst(types::I8, target as i64);

            // Pass num_regs instead of used_regs!
            flush_registers(bcx, ctx_ptr, val_vars, type_vars, num_regs);
            bcx.ins().call(helpers.load_string, &[ctx_ptr, str_ptr_val, str_len_val, target_val]);
            
            let new_val = load_memory_value(bcx, ctx_ptr, target);
            let new_type = load_memory_type(bcx, ctx_ptr, target);
            bcx.def_var(val_vars[target as usize], new_val);
            bcx.def_var(type_vars[target as usize], new_type);
        }
        MOV => {
            let target = (instruction >> 16) as u8;
            let arg = (instruction >> 8) as u8;

            let val = read_register_value(bcx, val_vars, arg);
            let rtype = read_register_type(bcx, type_vars, arg);

            write_register_dynamic(bcx, val_vars, type_vars, target, val, rtype);
        }
        GET_GLOBAL => {
            let target = (instruction >> 16) as u8;
            let name_pos = instruction as u16;
            let name = &chunk.string_constants[name_pos as usize];

            let name_ptr_val = bcx.ins().iconst(types::I64, name.as_ptr() as i64);
            let name_len_val = bcx.ins().iconst(types::I64, name.len() as i64);
            let target_val = bcx.ins().iconst(types::I8, target as i64);

            flush_registers(bcx, ctx_ptr, val_vars, type_vars, num_regs);
            bcx.ins().call(helpers.get_global, &[ctx_ptr, name_ptr_val, name_len_val, target_val]);
            
            let new_val = load_memory_value(bcx, ctx_ptr, target);
            let new_type = load_memory_type(bcx, ctx_ptr, target);
            bcx.def_var(val_vars[target as usize], new_val);
            bcx.def_var(type_vars[target as usize], new_type);
        }
        TRUE_IF_EQ => {
            let target = (instruction >> 16) as u8;
            let left = (instruction >> 8) as u8;
            let right = instruction as u8;
            
            let lval = read_register_value(bcx, val_vars, left);
            let ltype = read_register_type(bcx, type_vars, left);
            let rval = read_register_value(bcx, val_vars, right);
            let rtype = read_register_type(bcx, type_vars, right);

            let equal = check_equal(bcx, ltype, rtype, lval, rval);

            write_register(bcx, val_vars, type_vars, target, equal, Types::BOOL as u8);
        }
        TRUE_IF_NOTEQ => {
            let target = (instruction >> 16) as u8;
            let left = (instruction >> 8) as u8;
            let right = instruction as u8;

            let lval = read_register_value(bcx, val_vars, left);
            let ltype = read_register_type(bcx, type_vars, left);
            let rval = read_register_value(bcx, val_vars, right);
            let rtype = read_register_type(bcx, type_vars, right);

            let equal = check_equal(bcx, ltype, rtype, lval, rval);
            let not_equal = bcx.ins().bxor_imm(equal, 1);

            write_register(bcx, val_vars, type_vars, target, not_equal, Types::BOOL as u8);
        }
        JMP => {
            let offset = ((instruction << 8) as i32) >> 8; 
            let target_idx = (i as i32 + offset) as usize;

            let target_block = blocks[target_idx].expect("Block should have been created in pre-pass!");

            bcx.ins().jump(target_block, &[]);
            *terminated = true;
        }
        SKIP_ON_TRUE => {
            let target_reg = (instruction >> 16) as u8;
            
            let rtype = read_register_type(bcx, type_vars, target_reg);
            let val = read_register_value(bcx, val_vars, target_reg);

            let is_not_nil = bcx.ins().icmp_imm(IntCC::NotEqual, rtype, Types::NIL as i64);
            let is_not_bool = bcx.ins().icmp_imm(IntCC::NotEqual, rtype, Types::BOOL as i64);
            let is_one = bcx.ins().icmp_imm(IntCC::Equal, val, 1);

            let not_bool_or_one = bcx.ins().bor(is_not_bool, is_one);
            let skip_cond = bcx.ins().band(is_not_nil, not_bool_or_one);

            let skip_idx = i + 2;
            let fallthrough_idx = i + 1;

            let skip_block = blocks[skip_idx].unwrap();
            let fallthrough_block = blocks[fallthrough_idx].unwrap();

            bcx.ins().brif(
                skip_cond,            
                skip_block,         
                &[],                
                fallthrough_block,  
                &[]                 
            );
            
            *terminated = true;
        }
        CALL => {
            let target = (instruction >> 16) as u8;
            let start = (instruction >> 8) as u8;
            let arg_count = instruction as u8;

            let target_val = bcx.ins().iconst(types::I8, target as i64);
            let start_val = bcx.ins().iconst(types::I8, start as i64);
            let arg_count_val = bcx.ins().iconst(types::I8, arg_count as i64);

            flush_registers(bcx, ctx_ptr, val_vars, type_vars, num_regs);
            bcx.ins().call(helpers.call, &[ctx_ptr, target_val, start_val, arg_count_val]);
            
            reload_registers(bcx, ctx_ptr, val_vars, type_vars, num_regs);
        }
        RET => {
            flush_registers(bcx, ctx_ptr, val_vars, type_vars, num_regs);
            bcx.ins().return_(&[]);
            *terminated = true;
        }
        _ => panic!("Failed to JIT compile, no support for instruction {}", bytecode::opcode_name(opcode)),
    }
}