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
            Declaration { name, initial_value, scope: _ } => {
                let dest_var = self.new_var(name);
                
                // If there is an initial value, evaluate it and copy into dest_var
                if let Some(val) = initial_value {
                    let val_reg = self.compile_expression(val)?;
                    self.instructions.push(Copy { dest: dest_var, src: val_reg });
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

            Statement::Return { return_values } => {
                let mut compiled_expressions = Vec::with_capacity(return_values.len());
                for val in return_values {
                    compiled_expressions.push(self.compile_expression(val)?);
                }

                self.instructions.push(Return(compiled_expressions));
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

            // Handles assigment
            // TODO: disallow it as a generic expression (print(a = b)) 
            Expression::Assignment { target, value } => {
                match *target {
                    Expression::Variable(name) => {
                        let val_reg = self.compile_expression(*value)?;
                        self.set_variable(name, val_reg);
                        Ok(val_reg)
                    }
                    Expression::Field { object, index, computed: _ } => {
                        let val_reg = self.compile_expression(*value)?;
                        let idx_reg = self.compile_expression(*index)?;
                        let obj_reg = self.compile_expression(*object)?;
                        self.instructions.push(SetField { object: obj_reg, index: idx_reg, src: val_reg });

                        Ok(val_reg)
                    }
                    types => Err(CompileError::TypeError(format!("attempted to assign to {:?}", types))),
                }
            }

            // region:functions

            Expression::Call { callee, arguments } => {
                // Compile the callee expression
                let callee_reg = self.compile_expression(*callee)?;
                
                // Evaluate all arguments first
                let mut eval_arg_regs = Vec::new();
                for arg in arguments {
                    eval_arg_regs.push(self.compile_expression(arg)?);
                }

                // Now that all arguments are fully evaluated, allocate a contiguous block of temporary registers
                let mut contiguous_args = Vec::new();
                for arg_reg in eval_arg_regs {
                    let temp = self.new_temp();
                    
                    self.instructions.push(Copy { dest: temp, src: arg_reg });
                    contiguous_args.push(temp);
                }

                // Emit the Call instruction
                self.instructions.push(Call { callee: callee_reg, args: contiguous_args });

                // Handle the return value
                let dest = self.new_temp();
                self.instructions.push(GetReturn { dest, index: 0 });
                Ok(dest)
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

            exp => unimplemented!("expression {:#?}", exp),
        }
    }

    /// Compiles a program to IR and then to bytecode
    pub fn compile(&mut self, ast: Program) -> Result<Chunk, CompileError> {
        self.compile_program(ast)?;
        println!("{:#?}", self.instructions);
        let mut chunk = self.ir_generator.compile(std::mem::take(&mut self.instructions))?;
        chunk.array_indexing = 1;
        Ok(chunk)
    }
}