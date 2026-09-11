use crate::{bytecode::Chunk, errors::CompileError, ir::*};
use std::{collections::HashMap, println};

// CONSIDER: some sort of heuristic for pass vector allocation

#[derive(Default, Debug)]
pub struct IrGenerator();

impl IrGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    /// This pass turns pseudo-instructions into native instructions, for example `JumpIfFalse` would expand into `SkipOnTrue` and `Jump`.
    pub fn pass_extend(&mut self, instructions: Vec<IrInstruction>) -> Result<Vec<IrInstruction>, CompileError> {
        let mut out: Vec<IrInstruction> = Vec::with_capacity(instructions.len() * 2);
        for instruction in instructions {
            match instruction {
                IrInstruction::Array { dest, values } => {
                    out.push(IrInstruction::NewArray(dest));
                    for src in values {
                        out.push(IrInstruction::AppendArray { dest, src });
                    }
                }
                IrInstruction::Table { dest, values } => {
                    out.push(IrInstruction::NewTable(dest));
                    for src in values {
                        out.push(IrInstruction::AppendArray { dest, src });
                    }
                }
                IrInstruction::JumpIfFalse { dest, src } => {
                    out.push(IrInstruction::SkipOnTrue(src));
                    out.push(IrInstruction::Jump(dest));
                }
                IrInstruction::Return(return_values) => {
                    if return_values.is_empty() {
                        // Return 0 values (start can be 0 or any dummy register)
                        out.push(IrInstruction::Ret {
                            start: 0,
                            offset: 0,
                        });
                    } else if return_values.windows(2).all(|w| w[1].0 == w[0].0 + 1) {
                        // If the registers are already continious
                        out.push(IrInstruction::Ret {
                            start: return_values[0].0 as u8,
                            offset: return_values.len() as u8,
                        });
                    } else {
                        // Pick a start register safely above all source registers.
                        let max_src = return_values.iter().map(|r| r.0).max().unwrap();
                        let start = max_src + 1;

                        for (i, &src) in return_values.iter().enumerate() {
                            let dest = VirtualVar(start + i); // adjust cast to match your Register inner type
                            out.push(IrInstruction::Copy { dest, src });
                        }

                        out.push(IrInstruction::Ret {
                            start: start as u8,
                            offset: return_values.len() as u8,
                        });
                    }
                }
                any => out.push(any),
            }
        }

        Ok(out)
    }

    /// This pass directly converts the IR to bytecode. Note that it only supports native instructions and will throw an error on pseudo-instructions.
    // IMPORTANT: With the exception of labels, every instruction must add one thing to the chunk, no more or less. Otherwise jumps will break.
    pub fn pass_emit_bytecode(&mut self, instructions: Vec<IrInstruction>) -> Result<Chunk, CompileError> {
        let mut chunk = Chunk::new();

        // Maps labels
        let mut label_positions: HashMap<JumpLabel, usize> = HashMap::new();
        // Maps jump that need patching
        let mut forward_jumps: HashMap<JumpLabel, Vec<usize>> = HashMap::new();

        for instruction in instructions {
            let current_pos = chunk.code.len();

            match instruction {
                IrInstruction::SetGlobal { src, name } => chunk.add_set_global(&name, src.0 as u8),
                IrInstruction::GetGlobal { dest, name } => chunk.add_get_global(&name, dest.0 as u8),
                IrInstruction::LoadFloat { dest, value } => chunk.add_loadfloat(dest.0 as u8, value),
                IrInstruction::LoadString { dest, value } => chunk.add_loadstr(dest.0 as u8, &value),
                IrInstruction::LoadBool { dest, value } => chunk.add_loadbool(dest.0 as u8, value),
                IrInstruction::LoadNil(dest) => chunk.add_loadnil(dest.0 as u8),
                IrInstruction::Binary { dest, left, right , op} => {
                    match op {
                        Operator::Add => chunk.add_add(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Sub => chunk.add_min(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Mul => chunk.add_mul(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Div => chunk.add_div(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Pow => chunk.add_pow(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Concat => chunk.add_concat(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Eq => chunk.add_eq(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Ne => chunk.add_noteq(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Lt => chunk.add_lt(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Le => chunk.add_lteq(dest.0 as u8, left.0 as u8, right.0 as u8),
                        Operator::Gt => chunk.add_lt(dest.0 as u8, right.0 as u8, left.0 as u8),
                        Operator::Ge => chunk.add_lteq(dest.0 as u8, right.0 as u8, left.0 as u8),
                    }
                }
                IrInstruction::Copy { dest, src } => chunk.add_mov(dest.0 as u8, src.0 as u8),
                // CONSIDER: Move call to `pass_extend` and then make a low level version, similar to Return.
                IrInstruction::Call { callee, args } => {
                    let target = callee.0 as u8;
                    let arg_count = args.len() as u8;
                    
                    let start = args.first().map(|v| v.0).unwrap_or(0) as u8;
                    chunk.add_call(target, start, arg_count);
                }
                IrInstruction::GetReturn { dest, index } => chunk.add_get_ret(dest.0 as u8, index as u8),
                IrInstruction::SkipOnTrue(cond) => chunk.add_skip_on_true(cond.0 as u8),
                IrInstruction::Jump(label) => {
                    if let Some(target) = label_positions.get(&label) {
                        let offset = (*target as i32) - (current_pos as i32);
                        chunk.add_jump(offset);
                    } else {
                        forward_jumps.entry(label).or_default().push(current_pos);
                        chunk.add_jump(0); 
                    }
                }
                IrInstruction::Label(label) => {
                    label_positions.insert(label, current_pos);

                    if let Some(jumps) = forward_jumps.get(&label) {
                        for jump_pos in jumps {
                            let offset = (current_pos as i32) - (*jump_pos as i32);
                            
                            chunk.code[*jump_pos] = crate::bytecode::pack_u32_2x8_1x16(
                                crate::bytecode::Opcode::JMP as u8,
                                (offset >> 16) as u8,
                                offset as u16,
                            );
                        }
                    }
                }
                IrInstruction::LoadFunction { dest, function } => {
                    let compiled_code = IrGenerator::new().compile(function)?;
                    chunk.add_loadfunc(dest.0 as u8, compiled_code);
                }
                IrInstruction::SetField { object, index, src } => chunk.add_set_field(object.0 as u8, index.0 as u8, src.0 as u8),
                IrInstruction::GetField { dest, object, index } => chunk.add_get_field(dest.0 as u8, object.0 as u8, index.0 as u8),
                IrInstruction::Ret { start, offset } => chunk.add_ret(start as u8, offset as u8),
                IrInstruction::NewArray(dest) => chunk.add_newarray(dest.0 as u8),
                IrInstruction::NewTable(dest) => chunk.add_newtable(dest.0 as u8),
                IrInstruction::AppendArray { dest, src } => chunk.add_append_array(dest.0 as u8, src.0 as u8),
                f => unimplemented!("{:?}", f)
            }
        }

        Ok(chunk)
    }

    pub fn compile(&mut self, mut instructions: Vec<IrInstruction>) -> Result<Chunk, CompileError> {
        instructions = self.pass_extend(instructions)?;
        // let mut highest = 0usize;
        // for instruction in &mut instructions {
        //     instruction.visit_registers_mut(|reg, _access| {
        //         if reg.0 > highest {highest = reg.0}
        //     });
        // }
        // println!("highest register: {highest}");
        
        // Ensure that every function ends with RET to satisfy the VM
        if !matches!(instructions.last(), Some(IrInstruction::Ret { .. })) {
            instructions.push(IrInstruction::Ret { start: 0, offset: 0 });
        }

        self.pass_emit_bytecode(instructions)
    }
}