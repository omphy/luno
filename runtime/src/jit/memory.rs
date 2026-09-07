use cranelift::{prelude::*, frontend::Variable};

pub fn load_memory_type(bcx: &mut FunctionBuilder, ctx_ptr: Value, target: u8) -> Value {
    let fp = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 16);
    let tgt = bcx.ins().iconst(types::I64, target as i64);
    let abs_idx = bcx.ins().iadd(fp, tgt);

    let reg_types_ptr = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 8);
    let type_addr = bcx.ins().iadd(reg_types_ptr, abs_idx);
    bcx.ins().load(types::I8, MemFlagsData::new(), type_addr, 0)
}

pub fn load_memory_value(bcx: &mut FunctionBuilder, ctx_ptr: Value, target: u8) -> Value {
    let fp = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 16);
    let tgt = bcx.ins().iconst(types::I64, target as i64);
    let abs_idx = bcx.ins().iadd(fp, tgt);

    let reg_values_ptr = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 0);
    let reg_values_byte_offset = bcx.ins().ishl_imm(abs_idx, 3);
    let reg_addr = bcx.ins().iadd(reg_values_ptr, reg_values_byte_offset);
    bcx.ins().load(types::I64, MemFlagsData::new(), reg_addr, 0)
}

pub fn store_memory(bcx: &mut FunctionBuilder, ctx_ptr: Value, target: u8, val: Value, vtype: Value) {
    let fp = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 16);
    let tgt = bcx.ins().iconst(types::I64, target as i64);
    let abs_idx = bcx.ins().iadd(fp, tgt);

    let reg_values_ptr = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 0);
    let reg_values_byte_offset = bcx.ins().ishl_imm(abs_idx, 3);
    let reg_addr = bcx.ins().iadd(reg_values_ptr, reg_values_byte_offset);
    bcx.ins().store(MemFlagsData::new(), val, reg_addr, 0);

    let reg_types_ptr = bcx.ins().load(types::I64, MemFlagsData::new(), ctx_ptr, 8);
    let type_addr = bcx.ins().iadd(reg_types_ptr, abs_idx);
    bcx.ins().store(MemFlagsData::new(), vtype, type_addr, 0);
}

// Updated from used_regs to num_regs!
pub fn flush_registers(bcx: &mut FunctionBuilder, ctx_ptr: Value, val_vars: &[Variable], type_vars: &[Variable], num_regs: usize) {
    for i in 0..num_regs {
        let val = bcx.use_var(val_vars[i]);
        let vtype = bcx.use_var(type_vars[i]);
        store_memory(bcx, ctx_ptr, i as u8, val, vtype);
    }
}

// Updated from used_regs to num_regs!
pub fn reload_registers(bcx: &mut FunctionBuilder, ctx_ptr: Value, val_vars: &[Variable], type_vars: &[Variable], num_regs: usize) {
    for i in 0..num_regs {
        let val = load_memory_value(bcx, ctx_ptr, i as u8);
        let vtype = load_memory_type(bcx, ctx_ptr, i as u8);
        bcx.def_var(val_vars[i], val);
        bcx.def_var(type_vars[i], vtype);
    }
}