#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct JumpLabel(pub usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct VirtualVar(pub usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FunctionId(pub usize);

#[derive(Debug, Clone)]
pub enum Operator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Pow,

    // String / Array
    Concat,

    // Comparisons
    Eq, // ==
    Ne, // !=
    Lt, // <
    Le, // <=
    Gt, // >
    Ge, // >=
}

#[derive(Debug, Clone)]
pub enum IrInstruction {
    // Load
    LoadString { dest: VirtualVar, value: String },
    LoadFloat { dest: VirtualVar, value: f64 },
    LoadBool { dest: VirtualVar, value: bool },
    LoadFunction { dest: VirtualVar, function: Vec<IrInstruction> },
    LoadNil(VirtualVar),
    
    // Branching
    SkipOnTrue(VirtualVar),
    JumpIfFalse { dest: JumpLabel, src: VirtualVar },
    Jump(JumpLabel),
    Label(JumpLabel),
    
    // Functions
    Call { callee: VirtualVar, args: Vec<VirtualVar> },
    GetReturn { dest: VirtualVar, index: usize },
    Return(Vec<VirtualVar>),
    /// Low level version of `Return`
    Ret { start: u8, offset: u8 },
    
    // Globals
    GetGlobal { dest: VirtualVar, name: String },
    SetGlobal { src: VirtualVar, name: String },
    
    // Objects
    GetField { dest: VirtualVar, object: VirtualVar, index: VirtualVar },
    SetField { object: VirtualVar, index: VirtualVar, src: VirtualVar },
    Array {dest: VirtualVar, values: Vec<VirtualVar> },
    Table {dest: VirtualVar, values: Vec<VirtualVar> },
    NewArray(VirtualVar),
    NewTable(VirtualVar),
    AppendArray { dest: VirtualVar, src: VirtualVar },
    
    // Misc
    Binary { dest: VirtualVar, left: VirtualVar, right: VirtualVar, op: Operator },
    Copy { dest: VirtualVar, src: VirtualVar },
    Free(VirtualVar),
    Noop,
}

pub enum RegAccess {
    Def, // The register is being written to
    Use, // The register is being read from
}

impl IrInstruction {
    pub fn visit_registers_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut VirtualVar, RegAccess),
    {
        use RegAccess::*;
        match self {
            Self::Binary { dest, left, right, .. } => {
                f(dest, Def);
                f(left, Use);
                f(right, Use);
            }
            Self::SetField { object, index, src } => {
                f(object, Use);
                f(index, Use);
                f(src, Use);
            }
            Self::Copy { dest, src } => {
                f(dest, Def);
                f(src, Use);
            }
            Self::Array { dest, values } => {
                f(dest, Def);
                for val in values {
                    f(val, Use);
                }
            },
            Self::Table { dest, values } => {
                f(dest, Def);
                for val in values {
                    f(val, Use);
                }
            },
            Self::Call { callee, args } => {
                f(callee, Def);
                for arg in args {
                    f(arg, Use);
                }
            }
            Self::Return(values) => {
                for value in values {
                    f(value, Use);
                }
            }
            Self::Free(reg) => f(reg, Use),
            Self::GetField { dest, object, index } => {
                f(dest, Def);
                f(object, Use);
                f(index, Use);
            }
            Self::GetGlobal { dest, name: _ } => f(dest, Def),
            Self::GetReturn { dest, index: _ } => f(dest, Def),
            Self::JumpIfFalse { dest: _, src } => f(src, Use),
            Self::LoadBool { dest, value: _ } => f(dest, Def),
            Self::LoadFloat { dest, value: _ } => f(dest, Def),
            Self::LoadFunction { dest, function: _ } => f(dest, Def),
            Self::LoadNil(dest) => f(dest, Def),
            Self::LoadString { dest, value: _ } => f(dest, Def),
            Self::SetGlobal { src, name: _ } => f(src, Use),
            Self::SkipOnTrue(src) => f(src, Use),
            Self::Label(_) => {},
            Self::AppendArray { dest, src } => {
                f(dest, Def);
                f(src, Use);
            }
            Self::NewArray(dest) => f(dest, Def),
            Self::NewTable(dest) => f(dest, Def),
            Self::Noop => {},
            Self::Jump(_) => {},
            Self::Ret { start, offset } => {
                for reg in *start..=*start+*offset {
                    f(&mut VirtualVar(reg as usize), Use);
                }
            }
            // _ => {}
        }
    }
}