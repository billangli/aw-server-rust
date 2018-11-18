
#[derive(Debug)]
pub enum QueryError {
	// Lexing + Parsing
	LexingError, // FIXME: Lexing currently cannot fail without panic, unused
	ParsingError,

	// Execution
	VariableNotDefined(String),
	MathError(String),
	InvalidType(String),
}

mod lexer {
    use plex::lexer;

    #[derive(Debug, Clone)]
    pub enum Token {
        Ident(String),

        Return,

        Number(f64),
        String(String),
        Equals,
        Plus,
        Minus,
        Star,
        Slash,
        Percent,
        LParen,
        RParen,
        LBracket,
        RBracket,
        Comma,
        Semi,

        Whitespace,
        Comment,
    }

    lexer! {
        fn next_token(text: 'a) -> (Token, &'a str);

        r#"[ \t\r\n]+"# => (Token::Whitespace, text),
        // Python-style comments (# ...)
        r#"#[^\n]*"# => (Token::Comment, text),

        r#"return"# => (Token::Return, text),

		r#"\"[^\"]*\""# => (
			Token::String(text.to_owned()[1..text.len()-1].to_string()),
			text
		),
        r#"[0-9]+[\.]?[0-9]*"# => {
            (if let Ok(i) = text.parse() {
                Token::Number(i)
            } else {
                // TODO: do not panic, send an error
                panic!("integer {} is out of range", text)
            }, text)
        }

        r#"[a-zA-Z_][a-zA-Z0-9_]*"# => (Token::Ident(text.to_owned()), text),

        r#"="# => (Token::Equals, text),
        r#"\+"# => (Token::Plus, text),
        r#"-"# => (Token::Minus, text),
        r#"\*"# => (Token::Star, text),
        r#"/"# => (Token::Slash, text),
        r#"%"# => (Token::Percent, text),
        r#"\("# => (Token::LParen, text),
        r#"\)"# => (Token::RParen, text),
        r#"\["# => (Token::LBracket, text),
        r#"\]"# => (Token::RBracket, text),
        r#","# => (Token::Comma, text),
        r#";"# => (Token::Semi, text),

        // TODO: do not panic, send an error
        r#"."# => panic!("unexpected character: {}", text),
    }

    pub struct Lexer<'a> {
        original: &'a str,
        remaining: &'a str,
    }

    impl<'a> Lexer<'a> {
        pub fn new(s: &'a str) -> Lexer<'a> {
            Lexer { original: s, remaining: s }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Span {
        pub lo: usize,
        pub hi: usize,
    }

    fn span_in(s: &str, t: &str) -> Span {
        let lo = s.as_ptr() as usize - t.as_ptr() as usize;
        Span {
            lo: lo,
            hi: lo + s.len(),
        }
    }

    impl<'a> Iterator for Lexer<'a> {
        type Item = (Token, Span);
        fn next(&mut self) -> Option<(Token, Span)> {
            loop {
                let tok = if let Some((tok, new_remaining)) = next_token(self.remaining) {
                    self.remaining = new_remaining;
                    tok
                } else {
                    return None
                };
                match tok {
                    (Token::Whitespace, _) | (Token::Comment, _) => {
                        continue;
                    }
                    (tok, span) => {
                        return Some((tok, span_in(span, self.original)));
                    }
                }
            }
        }
    }
}

mod ast {
    use query::lexer::Span;

    #[derive(Debug)]
    pub struct Program {
        pub stmts: Vec<Expr>
    }

    #[derive(Debug,Clone)]
    pub struct Expr {
        pub span: Span,
        pub node: Expr_,
    }

    #[derive(Debug,Clone)]
    pub enum Expr_ {
        Add(Box<Expr>, Box<Expr>),
        Sub(Box<Expr>, Box<Expr>),
        Mul(Box<Expr>, Box<Expr>),
        Div(Box<Expr>, Box<Expr>),
        Mod(Box<Expr>, Box<Expr>),
        Var(String),
        Assign(String, Box<Expr>),
        // TODO: multi-argument functions
        Function(String, Box<Expr>),
        Return(Box<Expr>),
        Number(f64),
        String(String),
        List(Vec<Expr>),
    }
}

mod parser {
    use query::ast::*;
    use query::lexer::Token::*;
    use query::lexer::*;
    use plex::parser;
    parser! {
        fn parse_(Token, Span);

        // combine two spans
        (a, b) {
            Span {
                lo: a.lo,
                hi: b.hi,
            }
        }

        program: Program {
            statements[s] => Program { stmts: s }
        }

        statements: Vec<Expr> {
            => vec![],
            statements[mut st] ret[r] Semi => {
                st.push(r);
                st
            }
        }

        ret: Expr {
            Return assign[a] => Expr {
                span: span!(),
                node: Expr_::Return(Box::new(a)),
            },
            assign[a] => a
        }

        assign: Expr {
            Ident(fname) LParen assign[a] RParen => Expr {
                span: span!(),
                node: Expr_::Function(fname, Box::new(a)),
            },
            Ident(var) Equals assign[rhs] => Expr {
                span: span!(),
                node: Expr_::Assign(var, Box::new(rhs)),
            },
            object[o] => o
        }

        object: Expr {
            LBracket list[l] RBracket => l,
            LBracket RBracket => Expr {
                span: span!(),
                node: {
                    Expr_::List(Vec::new())
                }
            },
            term[o] => o,
        }

        list: Expr {
            object[o] => Expr {
                span: span!(),
                node: {
                    let mut list = Vec::new();
                    list.push(o);
                    Expr_::List(list)
                }
            },
            list[l] Comma object[o] => Expr {
                span: span!(),
                node: {
                    match l.node {
                        Expr_::List(mut l) => {
                            l.push(o);
                            // FIXME: this can be incredibly slow
                            Expr_::List(l.clone())
                        },
                        _ => panic!("a")
                    }
                }
            },
        }

        term: Expr {
            term[lhs] Plus fact[rhs] => Expr {
                span: span!(),
                node: Expr_::Add(Box::new(lhs), Box::new(rhs)),
            },
            term[lhs] Minus fact[rhs] => Expr {
                span: span!(),
                node: Expr_::Sub(Box::new(lhs), Box::new(rhs)),
            },
            fact[x] => x
        }

        fact: Expr {
            fact[lhs] Star atom[rhs] => Expr {
                span: span!(),
                node: Expr_::Mul(Box::new(lhs), Box::new(rhs)),
            },
            fact[lhs] Slash atom[rhs] => Expr {
                span: span!(),
                node: Expr_::Div(Box::new(lhs), Box::new(rhs)),
            },
            fact[lhs] Percent atom[rhs] => Expr {
                span: span!(),
                node: Expr_::Mod(Box::new(lhs), Box::new(rhs)),
            },
            atom[x] => x
        }

        atom: Expr {
            // round brackets to destructure tokens
            Ident(v) => Expr {
                span: span!(),
                node: Expr_::Var(v),
            },
            Number(i) => Expr {
                span: span!(),
                node: Expr_::Number(i),
            },
            String(s) => Expr {
                span: span!(),
                node: Expr_::String(s),
            },
            LParen assign[a] RParen => a
        }
    }

    pub fn parse<I: Iterator<Item=(Token, Span)>>(i: I) -> Result<Program, (Option<(Token, Span)>, &'static str)> {
        parse_(i)
    }
}

#[derive(Debug,Clone)]
pub enum DataType {
	None(),
	Number(f64),
	String(String),
	List(Vec<DataType>),
	Function(fn(Vec<DataType>) -> Result<DataType, QueryError>),
}

mod functions {
	use query::DataType;
	use query::QueryError;

	use std::collections::HashMap;

	pub fn fill_env<'a>(env: &mut HashMap<&'a str, DataType>) {
		env.insert("print", DataType::Function(q_print));
	}

	fn q_print(args: Vec<DataType>) -> Result<DataType, QueryError> {
		for arg in args {
			println!("{:?}", arg);
		}
		return Ok(DataType::None());
	}
}

mod interpret {
	use query;
    use query::ast::*;
	use query::DataType;
	use query::QueryError;
    use std::collections::HashMap;

	fn get_env<'a>() -> HashMap<&'a str, DataType> {
        let mut env = HashMap::new();
		query::functions::fill_env(&mut env);
		return env;
	}

    pub fn interpret_prog<'a>(p: &'a Program) -> Result<DataType, QueryError> {
		let last_i = p.stmts.len()-1;
		let mut env = get_env();
		let mut i = 0;
        for expr in &p.stmts {
            let ret = interpret_expr(&mut env, expr)?;
			// FIXME: This is ugly
			if i == last_i {
                return Ok(ret);
            }
			i+=1;
        }
        panic!("This should be unreachable!");
    }

    fn interpret_expr<'a>(env: &mut HashMap<&'a str, DataType>, expr: &'a Expr) -> Result<DataType, QueryError> {
        use query::ast::Expr_::*;
        match expr.node {
            Add(ref a, ref b) => {
                let a_res = interpret_expr(env, a)?;
                let b_res = interpret_expr(env, b)?;
                let a_num = match a_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot add something that is not a number!".to_string()))
                };
                let b_num = match b_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot add something that is not a number!".to_string()))
                };
                Ok(DataType::Number(a_num+b_num))
            },
            Sub(ref a, ref b) => {
                let a_res = interpret_expr(env, a)?;
                let b_res = interpret_expr(env, b)?;
                let a_num = match a_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                let b_num = match b_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                Ok(DataType::Number(a_num-b_num))
            },
            Mul(ref a, ref b) => {
                let a_res = interpret_expr(env, a)?;
                let b_res = interpret_expr(env, b)?;
                let a_num = match a_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                let b_num = match b_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                Ok(DataType::Number(a_num*b_num))
            },
            Div(ref a, ref b) => {
                let a_res = interpret_expr(env, a)?;
                let b_res = interpret_expr(env, b)?;
                let a_num = match a_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                let b_num = match b_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                if b_num == 0.0 {
                    return Err(QueryError::MathError("Tried to divide by zero!".to_string()));
                }
                Ok(DataType::Number(a_num/b_num))
            },
            Mod(ref a, ref b) => {
                let a_res = interpret_expr(env, a)?;
                let b_res = interpret_expr(env, b)?;
                let a_num = match a_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                let b_num = match b_res {
                    DataType::Number(n) => n,
                    _ => return Err(QueryError::InvalidType("Cannot sub something that is not a number!".to_string()))
                };
                Ok(DataType::Number(a_num%b_num))
            },
            Assign(ref var, ref b) => {
                let val = interpret_expr(env, b)?;
				// FIXME: avoid clone, it's slow
                env.insert(var, val.clone());
                Ok(val)
            }
			// FIXME: avoid clone, it's slow
            Var(ref var) => {
				match env.get(&var[..]) {
					Some(v) => Ok(v.clone()),
					None => Err(QueryError::VariableNotDefined(var.to_string()))
				}
			},
            Number(lit) => Ok(DataType::Number(lit)),
            String(ref litstr) => Ok(DataType::String(litstr.to_string())),
            Return(ref e) => {
                let val = interpret_expr(env, e)?;
                println!("{:?}", val);
				Ok(val)
            },
            Function(ref fname, ref e) => {
                let val = interpret_expr(env, e)?;
				let mut args = Vec::new();
				args.push(val);
                let var = match env.get(&fname[..]) {
                    Some(v) => v,
                    None => return Err(QueryError::VariableNotDefined(fname.clone()))
                };
				let f = match var {
					DataType::Function(f) => f,
					_ => return Err(QueryError::InvalidType(fname.to_string()))
				};
				f(args)
            },
            List(ref list) => {
                let mut l = Vec::new();
                for entry in list {
                    let res = interpret_expr(env, entry)?;
                    l.push(res);
                }
                Ok(DataType::List(l))
            }
        }
    }
}

pub fn query<'a>(code: &str) -> Result<DataType, QueryError> {
	let lexer = lexer::Lexer::new(code)
		.inspect(|tok| eprintln!("tok: {:?}", tok));
	let program = match parser::parse(lexer) {
		Ok(p) => p,
		Err(e) => {
			println!("{:?}", e);
			return Err(QueryError::ParsingError);
		}
	};
	interpret::interpret_prog(&program)
}