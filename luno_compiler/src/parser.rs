use crate::ast::{self, Expression, Statement};
use crate::token::{*, Token::*};
use std::string::String;
use logos::Logos;

// Parser struct
struct Parser<'a> {
    // Peekable iterator so we can look ahead at tokens without consuming them
    lexer: std::iter::Peekable<logos::Lexer<'a, Token>>,
}

impl<'a> Parser<'a> {
    // region:helpers

    /// Create a new parser
    fn new(source: &'a str) -> Self {
        Self {
            lexer: Token::lexer(source).peekable(),
        }
    }

    /// Get the next token without advancing
    fn peek(&mut self) -> Option<&Token> {
        match self.lexer.peek() {
            Some(Ok(token)) => Some(token),
            Some(Err(_)) => panic!("Lexer error: Encountered an unrecognized token!"),
            None => None,
        }
    }

    /// Get the next token while advancing
    fn advance(&mut self) -> Option<Token> {
        match self.lexer.next() {
            Some(Ok(token)) => Some(token),
            Some(Err(_)) => panic!("Lexer error: Encountered an unrecognized token!"),
            None => None,
        }
    }

    /// Check if the next token was the expected one without advancing
    fn check(&mut self, expected: &Token) -> bool {
        match self.peek() {
            Some(t) => t == expected,
            None => false,
        }
    }

    /// Advance and error if the token was an unexpected token
    fn consume(&mut self, expected: Token) -> Result<Token, String> {
        if self.check(&expected) {
            Ok(self.advance().unwrap())
        } else {
            Err(format!("Expected {:?}, found {:?}", expected, self.peek()))
        }
    }

    // endregion:helpers

    /// The very top level of any file
    fn parse_program(&mut self) -> ast::Program {
        let mut program = ast::Program::new();

        while self.peek().is_some() {
            program.statements.push(self.parse_statement());
        }

        program
    }

    /// Simply consumes an identifier 
    fn consume_identifier(&mut self) -> String {
        match self.advance() {
            Some(Token::Identifier(s)) => s,
            other => panic!("Expected an identifier, found {:?}", other),
        }
    }

    // region:pratt parser

    /// This is how much priority each infix (and post/prefix) has.<br>
    /// For example, in `1 + 2 * 3`, even though `+` comes earlier, `*` has a higher priority and will be nested deeper in the AST.
    fn infix_binding_power(token: &Token) -> Option<(u8, u8)> {
        match token {
            Token::Assign => Some((10, 9)),
            Token::Equal | Token::NotEqual => Some((16, 17)),
            Token::Less | Token::LessEq | Token::More | Token::MoreEq => Some((20, 21)),
            Token::Concat => Some((25, 26)),
            Token::Plus | Token::Minus => Some((30, 31)),
            Token::Multiply | Token::Divide => Some((40, 41)),
            Token::Exponent => Some((50, 51)),
            Token::ParenthesisOpen => Some((60, 61)),
            Token::SquareBracketOpen => Some((80, 81)), 
            Token::Dot => Some((80, 81)), 
            _ => None,
        }
    }

    /// Wrapper for parse_expression_bp with a min binding power of 0
    fn parse_expression(&mut self) -> Expression {
        self.parse_expression_bp(0)
    }

    /// Pratt parsing algorithm for parsing expressions
    fn parse_expression_bp(&mut self, min_bp: u8) -> Expression {
        let mut lhs = match self.advance() {
            // Number
            Some(Token::Number(n)) => Expression::Number(n),
            // Strings
            Some(Token::String(str)) => Expression::String(str),
            
            // Bools
            Some(Token::True) => Expression::Bool(true),
            Some(Token::False) => Expression::Bool(false),

            // Nil
            Some(Token::Nil) => Expression::Nil,

            // Variable
            Some(Token::Identifier(name)) => Expression::Variable(name),

            // Parenthesis
            Some(Token::ParenthesisOpen) => {
                // Parse the inner expression with 0 binding power to consume everything until the closing parenthesis no matter what
                let expr = self.parse_expression_bp(0);
                let _ = self
                    .consume(Token::ParenthesisClose)
                    .expect("Expected closing parenthesis");
                expr
            }
            // Inline function declaration (function() end)
            Some(Token::Function) => {
                // If there is a name, give it one
                let name = match self.peek() {
                    Some(Token::Identifier(_)) => {
                        if let Some(Token::Identifier(name)) = self.advance() {
                            Some(name)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                let mut params = Vec::new();

                let _ = self.consume(Token::ParenthesisOpen);
                // Parse all the parameters
                self.parse_parameter(&mut params);
                let _ = self.consume(Token::ParenthesisClose);

                // Parse code in the function
                let body = self.parse_block();

                Expression::FunctionDecl {
                    name,
                    params,
                    body: Box::new(body),
                }
            }
            // Arrays
            Some(Token::SquareBracketOpen) => {
                let mut elements = Vec::new();
                
                // Keep parsing elements until the closing bracket
                if self.peek() != Some(&Token::SquareBracketClose) {
                    loop {
                        elements.push(self.parse_expression());
                        
                        if self.check(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                
                self.consume(Token::SquareBracketClose)
                    .expect("expected closing bracket ']' after array elements");

                Expression::Array(elements)
            }

            Some(Token::CurlyBracketOpen) => {
                let mut elements = Vec::new();
                
                // Keep parsing elements until the closing bracket
                if self.peek() != Some(&Token::CurlyBracketClose) {
                    loop {
                        elements.push(self.parse_expression());
                        
                        if self.check(&Token::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                
                self.consume(Token::CurlyBracketClose)
                    .expect("expected closing bracket '}' after table elements");

                Expression::Table(elements)
            }
            // Unary minus (-value)
            Some(Token::Minus) => {
                let rhs = self.parse_expression_bp(70);
                Expression::Unary {
                    operator: "-".to_string(),
                    right: Box::new(rhs),
                }
            }
            other => panic!("Expected an expression, found {:?}", other),
        };

        // Pratt parsing loop
        while let Some(token) = self.peek() {
            // Break if this is the end
            let (left_bp, right_bp) = match Self::infix_binding_power(token) {
                Some(bp) => bp,
                None => break,
            };

            // Stop parsing if the next operator has lower precedence than the current minimum binding power
            if left_bp < min_bp {
                break;
            }

            let token = self.advance().unwrap();

            // You get the drift at this point
            match token {
                // Postfixes

                // Function calls
                Token::ParenthesisOpen => {
                    let mut arguments = Vec::new();
                    
                    // Get all the arguments
                    if self.peek() != Some(&Token::ParenthesisClose) {
                        loop {
                            arguments.push(self.parse_expression());
                            
                            if self.check(&Token::Comma) {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    
                    self.consume(Token::ParenthesisClose)
                    .expect("Expected closing parenthesis ')' after arguments");
                
                lhs = Expression::Call {
                        callee: Box::new(lhs),
                        arguments,
                    };
                }
                // Computed field access (obj[expr])
                Token::SquareBracketOpen => {
                    let index = self.parse_expression();
                    
                    self.consume(Token::SquareBracketClose)
                    .expect("Expected closing bracket ']' after accessing field");
                
                    lhs = Expression::Field {
                        object: Box::new(lhs),
                        index: Box::new(index),
                        computed: true
                    };
                    
                }
                // Uncomputed field access (obj.field)
                Token::Dot => {
                    let field = self.consume_identifier();
                
                    lhs = Expression::Field {
                        object: Box::new(lhs),
                        index: Box::new(Expression::String(field)),
                        computed: true
                    };
                    
                }
            
            // Infixes

            // Assignments
            Token::Assign => {
                let rhs = self.parse_expression_bp(right_bp);

                // Ensure we are only assigning to valid targets (variables or fields)
                match lhs {
                    Expression::Variable(_) | Expression::Field { .. } => {
                        lhs = Expression::Assignment {
                            target: Box::new(lhs),
                            value: Box::new(rhs),
                        }
                    }
                    _ => panic!("Invalid assignment target! Can only assign to variables or fields."),
                }
            }

            // Concat
            Token::Concat => {
                let operator = "..".to_string();
                let rhs = self.parse_expression_bp(right_bp);

                lhs = Expression::Binary {
                    left: Box::new(lhs),
                    operator,
                    right: Box::new(rhs),
                }
            }

            // Math operators
            _ => {
                    let operator = match token {
                        // Numbers
                        Token::Plus => "+".to_string(),
                        Token::Minus => "-".to_string(),
                        Token::Multiply => "*".to_string(),
                        Token::Divide => "/".to_string(),
                        Token::Exponent => "^".to_string(),

                        // Boolean
                        Token::Equal => "==".to_string(),
                        Token::NotEqual => "~=".to_string(),
                        Token::Less => "<".to_string(),
                        Token::LessEq => "<=".to_string(),
                        Token::More => ">".to_string(),
                        Token::MoreEq => ">=".to_string(),

                        _ => unreachable!(),
                    };

                    // Parse deeper into tree
                    let rhs = self.parse_expression_bp(right_bp);

                    lhs = Expression::Binary {
                        left: Box::new(lhs),
                        operator,
                        right: Box::new(rhs),
                    }
                }
            }
        }

        lhs
    }

    // endregion:pratt parser

    fn parse_parameter(&mut self, params: &mut Vec<String>) {
        match self.advance() {
            Some(Token::ParenthesisClose) => return,
            Some(Token::Identifier(name)) => params.push(name.to_string()),
            Some(token) => panic!("Error parsing parameters! Unknown token: {token:?}"),
            None => panic!("Error parsing parameters! No token found")
        };

        match self.peek() {
            Some(Token::ParenthesisClose) => return,
            Some(Token::Comma) => {
                self.advance();
                self.parse_parameter(params);
            },
            Some(token) => panic!("Error parsing parameters! Unknown token: {token:?}"),
            None => panic!("Error parsing parameters! No token found")
        };
    }

    fn parse_statement(&mut self) -> Statement {
        let token = self.peek();

        match token {
            // Function declaration
            Some(Token::Function) => Statement::Expr(self.parse_expression()),

            // Return
            Some(Token::Return) => {
                self.advance();
                let return_value = self.parse_expression();
                
                Statement::Return {
                    return_values: vec![return_value]
                }
            }
            // Local variable declaration
            Some(Token::Local) => {
                self.advance();
                self.parse_declaration(ast::Scope::Local)
            }
            // Global variable declaration
            Some(Token::Global) => {
                self.advance();
                self.parse_declaration(ast::Scope::Global)
            }
            // `do end` block 
            Some(Token::Do) => {
                self.advance();
                self.parse_block()
            }
            // `while ... do ... end` block
            Some(Token::While) => {
                self.advance();
                self.parse_while()
            }
            // `if ... then ... end`
            Some(Token::If) => {
                self.advance();
                self.parse_if()
            }
            // `var = ...` and `func()`
            Some(Token::Identifier(_)) => Statement::Expr(self.parse_expression()),
            other => panic!("Unexpected token {:?}", other),
        }
    }

    // local/global
    fn parse_declaration(&mut self, scope: ast::Scope) -> Statement {
        let name = self.consume_identifier();

        // Default to none
        let mut initial_value = None;
        
        // Check if the next token is `=`, so you can either do `local var` or `local var = val` 
        if let Some(token) = self.peek() {
            if *token == Assign {
                self.advance(); // Consumes the '=' token now that we know it's there
                initial_value = Some(self.parse_expression());
            }
        }

        Statement::Declaration {
            name,
            initial_value,
            scope,
        }
    }

    // Block like `do end`, `while do` and `if then`
    fn parse_block(&mut self) -> Statement {
        let mut statements = Vec::new();

        while self.peek().is_some() {
            // println!("{:?}", self.peek());
            let token = self.peek();

            // Stop parsing if we hit and `end`, `else` or `elseif`
            if token == Some(&Token::End) {
                self.advance();
                break;
            } else if token == Some(&Token::Else) || token == Some(&Token::ElseIf) {
                break;
            }

            // Compile line
            statements.push(self.parse_statement());
        }

        Statement::BlockScope { statements }
    }

    // While loops
    fn parse_while(&mut self) -> Statement {
        let condition = self.parse_expression();
        let _ = self.consume(Token::Do);
        let body = Box::new(self.parse_block());

        Statement::WhileLoop { condition, body }
    }

    // If statements
    fn parse_if(&mut self) -> Statement {
        let condition = self.parse_expression();
        let _ = self.consume(Token::Then);
        // Compile the main branch first
        let then_branch = Box::new(self.parse_block());

        // Compile any other branches
        let else_branch = match self.peek() {
            Some(Token::Else) => {
                self.advance();
                Some(Box::new(self.parse_block()))
            }
            // Nested if statement
            Some(Token::ElseIf) => {
                self.advance();
                Some(Box::new(self.parse_if()))
            }
            _ => None,
        };

        Statement::If {
            condition,
            then_branch,    
            else_branch,
        }
    }
}

pub fn parse_code(code: &str) -> ast::Program {
    // Temporarily do this before I add an actual error reporter
    let mut debug_lex = Token::lexer(code);
    while let Some(token) = debug_lex.next() {
        if token.is_err() {
            let span = debug_lex.span();

            // Grab a snippet of the code around the error so we can see the context
            let context_start = span.start.saturating_sub(15);
            let context_end = usize::min(code.len(), span.end.saturating_add(15));

            panic!(
                "Unrecognized character: '{}'\nAt index: {}\nContext: {:?}\n-----------------------\n",
                debug_lex.slice(),
                span.start,
                &code[context_start..context_end]
            );
        }
    }

    // Actually parse
    Parser::new(code).parse_program()
}
