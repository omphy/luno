use std::{collections::HashMap, format};
use runtime::{bytecode::Chunk, errors::CompileError, ir::{IrInstruction::{self, *}, *}, ir_generator::IrGenerator};
use crate::ast::{Statement::*, *};

#[derive(Debug, Default)]
pub struct Scope {
    locals: HashMap<String, VirtualVar>,
}

impl Scope {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub struct Compiler {
    instructions: Vec<IrInstruction>,
    ir_generator: IrGenerator,
    scopes: Vec<Scope>,
    next_var_id: usize,
    next_label_id: usize,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets a named variable from the local scope
    fn get_local(&self, name: &str) -> Option<VirtualVar> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.locals.get(name).copied())
    }

    /// Gets the current active scope
    fn _current_scope(&self) -> &Scope {
        self.scopes.last().expect("compiler bug: no active scope")
    }

    /// Gets the current active scope
    fn current_scope_mut(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("compiler bug: no active scope")
    }

    /// Generates a new temporary variable for the current scope
    fn new_temp(&mut self) -> VirtualVar {
        let id = VirtualVar(self.next_var_id);
        self.next_var_id += 1;
        self.current_scope_mut().locals.insert(format!("#temp{}", id.0), id);
        id
    }

    /// Creates a named variable in the current scope
    fn new_var(&mut self, name: String) -> VirtualVar {
        let id = VirtualVar(self.next_var_id);
        self.next_var_id += 1;
        self.current_scope_mut().locals.insert(name, id);
        id
    }

    /// Creates a new label in the current scope
    fn new_label(&mut self) -> JumpLabel {
        let id = JumpLabel(self.next_var_id);
        self.next_label_id += 1;
        id
    }

    /// Get a named variable, works for globals and locals
    fn get_variable(&mut self, name: String) -> VirtualVar {
        // If it's local just return its existing register
        if let Some(local_var) = self.get_local(&name) {
            return local_var;
        }

        // If it's global, we need a temp to load it at runtime
        let temp = self.new_temp();
        self.instructions.push(GetGlobal { dest: temp, name });
        temp
    }

    /// Write to a named variable, works for globals and locals
    fn set_variable(&mut self, name: String, src: VirtualVar) {
        if let Some(local_var) = self.get_local(&name) {
            // Local write, copy into the local variable's register
            self.instructions.push(Copy { dest: local_var, src });
        } else {
            // Global write, emit SetGlobal instruction
            self.instructions.push(SetGlobal { name, src });
        }
    }

    /// Compiles the program branch
    fn compile_program(&mut self, program: Program) -> Result<(), CompileError> {
        // Ensure there's a starting scope
        self.scopes.push(Scope::new());
        for stat in program.statements {
            self.compile_statement(stat)?;
        }

        // Pop the starting scope
        self.scopes.pop();
        Ok(())
    }

    /// Compiles a statement
    fn compile_statement(&mut self, stat: Statement) -> Result<(), CompileError> {
        match stat {
            Declaration { names, initial_values, scope: _ } => {
                if initial_values.is_empty() {
                    for name in names {
                        self.new_var(name);
                    }
                    return Ok(());
                }

                let mut compiled_rhs = Vec::new();
                
                let last_is_call = match initial_values.last() {
                    Some(Expression::Call { .. }) | Some(Expression::MethodCall { .. }) => true,
                    _ => false,
                };

                for val in initial_values {
                    compiled_rhs.push(self.compile_expression(val)?);
                }

                let diff = names.len().saturating_sub(compiled_rhs.len());
                
                if last_is_call && diff > 0 {
                    for i in 1..=diff {
                        let dest = self.new_temp();
                        self.instructions.push(GetReturn { dest, index: i as _ });
                        compiled_rhs.push(dest);
                    }
                } else {
                    for _ in 0..diff {
                        let dest = self.new_temp();
                        self.instructions.push(LoadNil(dest));
                        compiled_rhs.push(dest);
                    }
                }

                for (i, name) in names.into_iter().enumerate() {
                    let dest_var = self.new_var(name);
                    self.instructions.push(Copy { dest: dest_var, src: compiled_rhs[i] });
                }
            }

            Assignment { targets, values } => {
                let mut compiled_rhs = Vec::new();

                let last_is_call = match values.last() {
                    Some(Expression::Call { .. }) | Some(Expression::MethodCall { .. }) => true,
                    _ => false,
                };

                for val in values {
                    compiled_rhs.push(self.compile_expression(val)?);
                }

                let diff = targets.len().saturating_sub(compiled_rhs.len());
                
                if last_is_call && diff > 0 {
                    for i in 1..=diff {
                        let dest = self.new_temp();
                        self.instructions.push(GetReturn { dest, index: i as _ });
                        compiled_rhs.push(dest);
                    }
                } else {
                    for _ in 0..diff {
                        let dest = self.new_temp();
                        self.instructions.push(LoadNil(dest));
                        compiled_rhs.push(dest);
                    }
                }

                for (i, target) in targets.into_iter().enumerate() {
                    let val_reg = compiled_rhs[i];
                    match target {
                        Expression::Variable(name) => {
                            self.set_variable(name, val_reg);
                        }
                        Expression::Field { object, index, computed: _ } => {
                            let obj_reg = self.compile_expression(*object)?;
                            let idx_reg = self.compile_expression(*index)?;
                            self.instructions.push(SetField { object: obj_reg, index: idx_reg, src: val_reg });
                        }
                        _ => return Err(CompileError::TypeError(format!("attempted to assign to invalid target"))),
                    }
                }
            }

            // Expression statements (function ...() end, func(), foo = bar)
            Expr(ex) => {
                self.compile_expression(ex)?;
            }

            BlockScope { statements } => {
                // Create a new scope
                self.scopes.push(Scope::new());
                
                for stat in statements {
                    self.compile_statement(stat)?;
                }

                // Pop the new scope
                self.scopes.pop();
            }
            
            If { condition, then_branch, else_branch } => {
                let cond_reg = self.compile_expression(condition)?;

                let else_label = self.new_label();
                self.instructions.push(JumpIfFalse { dest: else_label, src: cond_reg });
                self.compile_statement(*then_branch)?;
                
                if let Some(else_stmt) = else_branch {
                    let end_label = self.new_label();

                    // if the if was true, skip the else branch
                    self.instructions.push(Jump(end_label));
                    self.instructions.push(Label(else_label));
                    
                    self.compile_statement(*else_stmt)?;

                    self.instructions.push(Label(end_label));
                } else {
                    self.instructions.push(Label(else_label));
                }
            }

            WhileLoop { condition, body } => {
                let start_label = self.new_label();
                self.instructions.push(Label(start_label));

                let cond = self.compile_expression(condition)?;
                let end_label = self.new_label();
                self.instructions.push(JumpIfFalse{dest: end_label, src: cond});
                
                self.compile_statement(*body)?;

                self.instructions.push(Jump(start_label));
                self.instructions.push(Label(end_label));
            }

            Statement::Return { mut return_values } => {
                let mut compiled_expressions = Vec::new();
                let mut expand_last = false;

                if let Some(last_expr) = return_values.pop() {
                    for val in return_values {
                        compiled_expressions.push(self.compile_expression(val)?);
                    }

                    match last_expr {
                        Expression::Call { .. } | Expression::MethodCall { .. } => {
                            self.compile_invocation(last_expr, false)?;
                            expand_last = true;
                        }
                        _ => {
                            compiled_expressions.push(self.compile_expression(last_expr)?);
                        }
                    }
                }

                self.instructions.push(IrInstruction::Return { values: compiled_expressions, multret: expand_last });
            }

            s => panic!("no support for '{:?}'", s),
        }
        Ok(())
    }

    /// Compiles the expression branch
    fn compile_expression(&mut self, expr: Expression) -> Result<VirtualVar, CompileError> {
        match expr {
            // region:variables
            // Constansts
            Expression::Number(f) => {
                let dest = self.new_temp();
                self.instructions.push(LoadFloat { dest, value: f });
                Ok(dest)
            }
            Expression::Bool(b) => {
                let dest = self.new_temp();
                self.instructions.push(LoadBool { dest, value: b });
                Ok(dest)
            }
            Expression::Nil => {
                let dest = self.new_temp();
                self.instructions.push(LoadNil(dest));
                Ok(dest)
            }
            Expression::String(s) => {
                let dest = self.new_temp();
                self.instructions.push(LoadString { dest, value: s });
                Ok(dest)
            }

            // Variable
            Expression::Variable(name) => {
                Ok(self.get_variable(name))
            }
            // endregion:variables

            // Handles binary math operators
            Expression::Binary { left, operator, right } => {
                // Evaluate left and right — each just returns the register it lives in!
                let left_reg = self.compile_expression(*left)?;
                let right_reg = self.compile_expression(*right)?;

                let dest = self.new_temp();
                match operator.as_str() {
                    "+" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Add }),
                    "-" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Sub }),
                    "*" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Mul }),
                    "/" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Div }),
                    "^" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Pow }),
                    ".." => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Concat }),
                    "==" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Eq }),
                    "~=" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Ne }),
                    "<" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Lt }),
                    "<=" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Le }),
                    ">" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Gt }),
                    ">=" => self.instructions.push(Binary { dest, left: left_reg, right: right_reg, op: Operator::Ge }),
                    _ => unimplemented!(),
                }
                Ok(dest)
            }

            // region:functions

            Expression::Call { .. } | Expression::MethodCall { .. } => {
                self.compile_invocation(expr, true)
            }

            Expression::FunctionDecl { name, params, body } => {
                let fn_reg;
                if let Some(fn_name) = name {
                    fn_reg = self.new_var(fn_name);
                } else {
                    fn_reg = self.new_temp();
                }

                let mut function = Self::new();

                function.scopes.push(Scope::new());
                for param in params {
                    function.new_var(param);
                }
                function.compile_statement(*body)?;
                function.scopes.pop();

                self.instructions.push(LoadFunction { dest: fn_reg, function: function.instructions });

                Ok(fn_reg)
            }

            // endregion:functions
            // region:objects
            Expression::Field { object, index, computed: _ } => {
                let temp = self.new_temp();

                let obj = self.compile_expression(*object)?;
                let idx = self.compile_expression(*index)?;

                self.instructions.push(GetField { dest: temp, object: obj, index: idx });
                Ok(temp)
            }

            Expression::Array(values) => {
                let array = self.new_temp();
                let mut compiled_expressions = Vec::with_capacity(values.len());
                for val in values {
                    compiled_expressions.push(self.compile_expression(val)?);
                }

                self.instructions.push(Array { dest: array, values: compiled_expressions });

                Ok(array)
            }

            Expression::Table(values) => {
                let table = self.new_temp();
                let mut compiled_expressions = Vec::with_capacity(values.len());
                for val in values {
                    compiled_expressions.push(self.compile_expression(val)?);
                }

                self.instructions.push(Table { dest: table, values: compiled_expressions });

                Ok(table)
            }

            // endregion:objects

            Expression::Unary { operator, right } => {
                match operator.as_str() {
                    "-" => {
                        let zero = self.new_temp();
                        self.instructions.push(LoadFloat { dest: zero, value: 0. });
                        
                        let val = self.compile_expression(*right)?;

                        let new = self.new_temp();
                        self.instructions.push(Binary { dest: new, left: zero, right: val, op: Operator::Sub } );

                        Ok(new)
                    }
                    exp => unimplemented!("expression {:?}", exp),
                }
            }

            // exp => unimplemented!("expression {:#?}", exp),
        }
    }

    fn compile_invocation(&mut self, expr: Expression, truncate: bool) -> Result<VirtualVar, CompileError> {
        // ultra effecient code
        match expr {
            Expression::Call { callee, mut arguments } => {
                let callee_reg = self.compile_expression(*callee)?;
                let mut eval_arg_regs = Vec::new();
                let mut expand_last = false;

                if let Some(last_expr) = arguments.pop() {
                    for arg in arguments {
                        eval_arg_regs.push(self.compile_expression(arg)?);
                    }
                    match last_expr {
                        Expression::Call { .. } | Expression::MethodCall { .. } => {
                            self.compile_invocation(last_expr, false)?;
                            expand_last = true;
                        }
                        _ => {
                            eval_arg_regs.push(self.compile_expression(last_expr)?);
                        }
                    }
                }

                let mut contiguous_args = Vec::new();
                for arg_reg in eval_arg_regs {
                    let temp = self.new_temp();
                    self.instructions.push(Copy { dest: temp, src: arg_reg });
                    contiguous_args.push(temp);
                }

                self.instructions.push(Call { callee: callee_reg, args: contiguous_args, multret: expand_last });

                let dest = self.new_temp();
                if truncate {
                    self.instructions.push(GetReturn { dest, index: 0 });
                }
                Ok(dest)
            }
            Expression::MethodCall { object, method_name, mut arguments } => {
                let obj_reg = self.compile_expression(*object)?;

                let method_str_reg = self.new_temp();
                self.instructions.push(LoadString { dest: method_str_reg, value: method_name });

                let callee_reg = self.new_temp();
                self.instructions.push(GetField { dest: callee_reg, object: obj_reg, index: method_str_reg });

                let mut eval_arg_regs = Vec::new();
                let mut expand_last = false;

                if let Some(last_expr) = arguments.pop() {
                    for arg in arguments {
                        eval_arg_regs.push(self.compile_expression(arg)?);
                    }
                    match last_expr {
                        Expression::Call { .. } | Expression::MethodCall { .. } => {
                            self.compile_invocation(last_expr, false)?;
                            expand_last = true;
                        }
                        _ => {
                            eval_arg_regs.push(self.compile_expression(last_expr)?);
                        }
                    }
                }

                let mut contiguous_args = Vec::new();
                let self_temp = self.new_temp();
                self.instructions.push(Copy { dest: self_temp, src: obj_reg });
                contiguous_args.push(self_temp);

                for arg_reg in eval_arg_regs {
                    let temp = self.new_temp();
                    self.instructions.push(Copy { dest: temp, src: arg_reg });
                    contiguous_args.push(temp);
                }

                self.instructions.push(Call { callee: callee_reg, args: contiguous_args, multret: expand_last });

                let dest = self.new_temp();
                if truncate {
                    self.instructions.push(GetReturn { dest, index: 0 });
                }
                Ok(dest)
            }
            _ => unreachable!(),
        }
    }

    /// Compiles a program to IR and then to bytecode
    pub fn compile(&mut self, ast: Program) -> Result<Chunk, CompileError> {
        self.compile_program(ast)?;
        // println!("{:#?}", self.instructions);
        let mut chunk = self.ir_generator.compile(std::mem::take(&mut self.instructions))?;
        chunk.array_indexing = 1;
        Ok(chunk)
    }
}