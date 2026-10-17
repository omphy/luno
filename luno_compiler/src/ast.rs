#![allow(dead_code)]

// The entire program
#[derive(Debug)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Program {
            statements: Vec::new(),
        }
    }
}

// Statements like declarations, while loops, if statements, most keywords, etc
#[derive(Debug)]
pub enum Statement {
    Expr(Expression),
    Declaration {
        names: Vec<String>,
        initial_values: Vec<Expression>,
        scope: Scope,
    },
    Assignment {
        targets: Vec<Expression>,
        values: Vec<Expression>,
    },
    BlockScope {
        statements: Vec<Statement>,
    },
    WhileLoop {
        condition: Expression,
        body: Box<Statement>,
    },
    If {
        condition: Expression,
        then_branch: Box<Statement>,
        else_branch: Option<Box<Statement>>,
    },
    GuestBlock {
        language: String,
        source_code: String,
    },
    Return {
        return_values: Vec<Expression>
    }
}

// The scope a variable declaration can have
#[derive(Debug)]
pub enum Scope {
    Local,
    Global,
}

// Expressions like math and assignments
#[derive(Debug)]
pub enum Expression {
    Number(f64),
    Variable(String),
    String(String),
    Bool(bool),
    Nil,
    Array(Vec<Expression>),
    Table(Vec<Expression>),
    Binary {
        left: Box<Expression>,
        operator: String,
        right: Box<Expression>,
    },
    FunctionDecl {
        name: Option<String>,
        params: Vec<String>,
        body: Box<Statement>,
    },
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    Field {
        object: Box<Expression>,
        index: Box<Expression>,
        computed: bool, // computed = obj.field non-computed = obj[field]
    },
    Unary {
        operator: String,
        right: Box<Expression>,
    },

    // this is for foo:bar(), the reason this is seperate from `Call` is because
    // while foo:bar() becoming foo:bar(foo) is correct, foo():bar() becoming foo():bar(foo()) is not
    MethodCall { 
        object: Box<Expression>, 
        method_name: String, 
        arguments: Vec<Expression> 
    },
}
