use enumset::EnumSet;

use crate::{
    lex::{
        TokenId,
        TokenKind::{self, *},
        Tokens,
    },
    util::u32_to_usize,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeId {
    pub index: u32,
}

impl From<TypeId> for usize {
    fn from(id: TypeId) -> Self {
        u32_to_usize(id.index)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BindId {
    pub index: u32,
}

impl From<BindId> for usize {
    fn from(id: BindId) -> Self {
        u32_to_usize(id.index)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExprId {
    pub index: u32,
}

impl From<ExprId> for usize {
    fn from(id: ExprId) -> Self {
        u32_to_usize(id.index)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Type {
    Unit,
    Name { name: TokenId },
    Pair { fst: TypeId, snd: TypeId },
}

#[derive(Clone, Copy, Debug)]
pub enum Bind {
    Unit,
    Name { name: TokenId },
    Pair { fst: Param, snd: Param },
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub bind: BindId,
    pub ty: Option<TypeId>,
}

#[derive(Clone, Copy, Debug)]
pub enum Binop {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug)]
pub enum Expr {
    Name {
        name: TokenId,
    },
    Unit,
    Number {
        val: TokenId,
    },
    Pair {
        fst: ExprId,
        snd: ExprId,
    },
    Apply {
        func: ExprId,
        arg: ExprId,
    },
    Let {
        param: Param,
        val: ExprId,
        body: ExprId,
    },
    Binary {
        lhs: ExprId,
        op: Binop,
        rhs: ExprId,
    },
}

#[derive(Debug)]
pub struct Def {
    pub name: TokenId,
    pub params: Vec<Param>,
    pub ty: Option<TypeId>,
    pub body: ExprId,
}

#[derive(Debug)]
pub struct Module {
    types: Vec<Type>,
    binds: Vec<Bind>,
    exprs: Vec<Expr>,
    defs: Vec<Def>,
}

impl Module {
    fn make_ty(&mut self, ty: Type) -> TypeId {
        let id = TypeId {
            index: self.types.len().try_into().unwrap(),
        };
        self.types.push(ty);
        id
    }

    fn make_bind(&mut self, bind: Bind) -> BindId {
        let id = BindId {
            index: self.binds.len().try_into().unwrap(),
        };
        self.binds.push(bind);
        id
    }

    fn make_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId {
            index: self.exprs.len().try_into().unwrap(),
        };
        self.exprs.push(expr);
        id
    }

    pub fn ty(&self, id: TypeId) -> Type {
        self.types[usize::from(id)]
    }

    pub fn bind(&self, id: BindId) -> Bind {
        self.binds[usize::from(id)]
    }

    pub fn expr(&self, id: ExprId) -> Expr {
        self.exprs[usize::from(id)]
    }

    pub fn defs(&self) -> &[Def] {
        &self.defs
    }
}

#[derive(Debug)]
pub enum ParseError {
    Expected {
        id: TokenId,
        kinds: EnumSet<TokenKind>,
    },
}

#[derive(Debug)]
struct Parser<'a> {
    tokens: &'a Tokens,
    before_ws: TokenId,
    id: TokenId,
    module: Module,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> TokenKind {
        self.tokens.get(self.id).kind
    }

    fn find_non_ws(&mut self) {
        while let Newline | Comment = self.peek() {
            self.id.index += 1;
        }
    }

    fn next(&mut self) {
        if let Eof = self.peek() {
            panic!("unexpected end of file");
        }
        self.before_ws = TokenId {
            index: self.id.index + 1,
        };
        self.id = self.before_ws;
        self.find_non_ws();
    }

    fn expect(&mut self, kind: TokenKind) -> Result<TokenId, ParseError> {
        let id = self.id;
        if self.peek() == kind {
            self.next();
            Ok(id)
        } else {
            Err(ParseError::Expected {
                id,
                kinds: EnumSet::only(kind),
            })
        }
    }

    fn newline(&self) -> bool {
        // we only allow single-line comments, so anything ignored must include a newline
        self.before_ws < self.id
    }

    fn ty_atom(&mut self) -> Result<TypeId, ParseError> {
        match self.peek() {
            Ident => {
                let name = self.id;
                self.next();
                Ok(self.module.make_ty(Type::Name { name }))
            }
            LParen => {
                self.next();
                match self.peek() {
                    RParen => {
                        self.next();
                        Ok(self.module.make_ty(Type::Unit))
                    }
                    _ => {
                        let ty = self.ty()?;
                        self.expect(RParen)?;
                        Ok(ty)
                    }
                }
            }
            _ => Err(ParseError::Expected {
                id: self.id,
                kinds: Ident | LParen,
            }),
        }
    }

    fn ty_elem(&mut self) -> Result<TypeId, ParseError> {
        self.ty_atom()
    }

    fn ty(&mut self) -> Result<TypeId, ParseError> {
        let mut types = vec![self.ty_elem()?];
        while let Comma = self.peek() {
            self.next();
            types.push(self.ty_elem()?);
        }
        let last = types.pop().unwrap();
        Ok(types.into_iter().rfold(last, |snd, fst| {
            self.module.make_ty(Type::Pair { fst, snd })
        }))
    }

    fn bind_atom(&mut self) -> Result<BindId, ParseError> {
        match self.peek() {
            Ident => {
                let name = self.id;
                self.next();
                Ok(self.module.make_bind(Bind::Name { name }))
            }
            LParen => {
                self.next();
                match self.peek() {
                    RParen => {
                        self.next();
                        Ok(self.module.make_bind(Bind::Unit))
                    }
                    _ => {
                        let Param { bind, ty } = self.param()?;
                        let right = self.expect(RParen)?;
                        match ty {
                            Some(_) => Err(ParseError::Expected {
                                id: right,
                                kinds: EnumSet::only(Comma),
                            }),
                            None => Ok(bind),
                        }
                    }
                }
            }
            _ => Err(ParseError::Expected {
                id: self.id,
                kinds: Ident | LParen,
            }),
        }
    }

    fn bind_elem(&mut self) -> Result<BindId, ParseError> {
        self.bind_atom()
    }

    fn param_elem(&mut self) -> Result<Param, ParseError> {
        let bind = self.bind_elem()?;
        let ty = match self.peek() {
            Colon => {
                self.next();
                Some(self.ty_elem()?)
            }
            _ => None,
        };
        Ok(Param { bind, ty })
    }

    fn param(&mut self) -> Result<Param, ParseError> {
        let mut params = vec![self.param_elem()?];
        while let Comma = self.peek() {
            self.next();
            params.push(self.param_elem()?);
        }
        let last = params.pop().unwrap();
        Ok(params.into_iter().rfold(last, |snd, fst| Param {
            bind: self.module.make_bind(Bind::Pair { fst, snd }),
            ty: None,
        }))
    }

    fn expr_atom(&mut self) -> Result<ExprId, ParseError> {
        match self.peek() {
            LParen => {
                self.next();
                match self.peek() {
                    RParen => {
                        self.next();
                        Ok(self.module.make_expr(Expr::Unit))
                    }
                    _ => {
                        let expr = self.expr()?;
                        self.expect(RParen)?;
                        Ok(expr)
                    }
                }
            }
            Ident => {
                let name = self.id;
                self.next();
                Ok(self.module.make_expr(Expr::Name { name }))
            }
            Number => {
                let val = self.id;
                self.next();
                Ok(self.module.make_expr(Expr::Number { val }))
            }
            _ => Err(ParseError::Expected {
                id: self.id,
                kinds: LParen | Ident | Number,
            }),
        }
    }

    fn expr_factor(&mut self) -> Result<ExprId, ParseError> {
        let mut f = self.expr_atom()?;
        // function application is the only place we forbid line breaks
        while !self.newline() {
            match self.peek() {
                // same set of tokens allowed at the start of an atomic expression
                LParen | Ident | Number => {
                    let x = self.expr_atom()?;
                    f = self.module.make_expr(Expr::Apply { func: f, arg: x });
                }
                _ => break,
            }
        }
        Ok(f)
    }

    fn expr_term(&mut self) -> Result<ExprId, ParseError> {
        let mut lhs = self.expr_factor()?;
        loop {
            let op = match self.peek() {
                Asterisk => Binop::Mul,
                Slash => Binop::Div,
                _ => break,
            };
            self.next();
            let rhs = self.expr_factor()?;
            lhs = self.module.make_expr(Expr::Binary { lhs, op, rhs });
        }
        Ok(lhs)
    }

    fn expr_elem(&mut self) -> Result<ExprId, ParseError> {
        let mut lhs = self.expr_term()?;
        loop {
            let op = match self.peek() {
                Plus => Binop::Add,
                Hyphen => Binop::Sub,
                _ => break,
            };
            self.next();
            let rhs = self.expr_term()?;
            lhs = self.module.make_expr(Expr::Binary { lhs, op, rhs });
        }
        Ok(lhs)
    }

    fn expr_inner(&mut self) -> Result<ExprId, ParseError> {
        let mut exprs = vec![self.expr_elem()?];
        while let Comma = self.peek() {
            self.next();
            exprs.push(self.expr_elem()?);
        }
        let last = exprs.pop().unwrap();
        Ok(exprs.into_iter().rfold(last, |snd, fst| {
            self.module.make_expr(Expr::Pair { fst, snd })
        }))
    }

    fn expr(&mut self) -> Result<ExprId, ParseError> {
        match self.peek() {
            Let => {
                self.next();
                let param = self.param()?;
                self.expect(Equal)?;
                let val = self.expr_inner()?;
                if !self.newline() {
                    self.expect(Semicolon)?;
                }
                let body = self.expr()?;
                Ok(self.module.make_expr(Expr::Let { param, val, body }))
            }
            _ => self.expr_inner(),
        }
    }

    fn def(&mut self) -> Result<Def, ParseError> {
        self.expect(Def)?;
        let name = self.expect(Ident)?;
        let mut params = vec![];
        while let LParen = self.peek() {
            self.next();
            match self.peek() {
                RParen => {
                    self.next();
                    let bind = self.module.make_bind(Bind::Unit);
                    params.push(Param { bind, ty: None });
                }
                _ => {
                    params.push(self.param()?);
                    self.expect(RParen)?;
                }
            }
        }
        let ty = match self.peek() {
            Colon => {
                self.next();
                Some(self.ty()?)
            }
            _ => None,
        };
        self.expect(Equal)?;
        let body = self.expr()?;
        Ok(Def {
            name,
            params,
            ty,
            body,
        })
    }

    fn module(mut self) -> Result<Module, ParseError> {
        loop {
            match self.peek() {
                Def => {
                    let def = self.def()?;
                    self.module.defs.push(def);
                }
                Eof => return Ok(self.module),
                _ => {
                    return Err(ParseError::Expected {
                        id: self.id,
                        kinds: Def | Eof,
                    })
                }
            }
        }
    }
}

pub fn parse(tokens: &Tokens) -> Result<Module, ParseError> {
    let id = TokenId { index: 0 };
    let mut parser = Parser {
        tokens,
        before_ws: id,
        id,
        module: Module {
            types: vec![],
            binds: vec![],
            exprs: vec![],
            defs: vec![],
        },
    };
    parser.find_non_ws();
    parser.module()
}
