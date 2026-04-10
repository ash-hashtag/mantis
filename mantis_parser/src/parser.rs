use crate::ast::*;
use crate::token::{Span, SpannedToken, Token};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Parser state
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

type PResult<T> = Result<T, ParseError>;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at {}: {}", self.span, self.message)
    }
}

impl std::error::Error for ParseError {}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.token)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or(Span::new(0, 0))
    }

    fn peek_nth(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n).map(|t| &t.token)
    }

    fn advance(&mut self) -> &SpannedToken {
        let tok = &self.tokens[self.pos];
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> PResult<Span> {
        if let Some(tok) = self.peek() {
            if std::mem::discriminant(tok) == std::mem::discriminant(expected) {
                let sp = self.tokens[self.pos].span;
                self.pos += 1;
                return Ok(sp);
            }
            return Err(self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                tok
            )));
        }
        Err(self.error(format!("expected {:?}, found EOF", expected)))
    }

    fn expect_ident(&mut self) -> PResult<Ident> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                let span = self.advance().span;
                Ok(Ident::new(name, span))
            }
            other => Err(self.error(format!("expected identifier, found {:?}", other))),
        }
    }

    fn eat(&mut self, expected: &Token) -> bool {
        if let Some(tok) = self.peek() {
            if std::mem::discriminant(tok) == std::mem::discriminant(expected) {
                self.pos += 1;
                return true;
            }
        }
        false
    }

    fn error(&self, message: String) -> ParseError {
        ParseError {
            message,
            span: self.peek_span(),
        }
    }

    fn prev_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::new(0, 0)
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Top-level: parse program
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut declarations = Vec::new();
        while !self.at_end() {
            declarations.push(self.parse_declaration()?);
        }
        Ok(Program { declarations })
    }

    fn parse_declaration(&mut self) -> PResult<Declaration> {
        match self.peek() {
            Some(Token::Fn) => Ok(Declaration::Function(self.parse_fn_decl()?)),
            Some(Token::Type) => Ok(Declaration::TypeDef(self.parse_type_def()?)),
            Some(Token::Import) => Ok(Declaration::Import(self.parse_import()?)),
            Some(Token::Use) => Ok(Declaration::Use(self.parse_use()?)),
            Some(Token::Trait) => Ok(Declaration::Trait(self.parse_trait()?)),
            Some(Token::Impl) => Ok(Declaration::Impl(self.parse_impl()?)),
            _ => Err(self.error(format!(
                "expected declaration (fn, type, import, use, trait, impl), found {:?}",
                self.peek()
            ))),
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Import
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_import(&mut self) -> PResult<ImportDecl> {
        let start = self.expect(&Token::Import)?;
        
        // Support either an identifier path or a string literal path
        let mut path = Vec::new();
        if let Some(Token::String(s)) = self.peek().cloned() {
            self.advance();
            path.push(Ident::new(s, self.prev_span()));
        } else {
            path.push(self.expect_ident()?);
            while self.eat(&Token::Dot) {
                path.push(self.expect_ident()?);
            }
        }

        // Support optional 'as *' or 'as alias'
        let mut alias = None;
        if self.eat(&Token::As) {
            match self.peek() {
                Some(Token::Star) => {
                    self.advance();
                    alias = Some(Ident::new("*", self.prev_span()));
                }
                Some(Token::Ident(_)) => {
                    alias = Some(self.expect_ident()?);
                }
                _ => return Err(self.error("expected '*' or identifier after 'as'".into())),
            }
        }

        self.expect(&Token::Semi)?;
        let span = start.merge(self.prev_span());
        Ok(ImportDecl { path, alias, span })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Use
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_use(&mut self) -> PResult<UseDecl> {
        let start = self.expect(&Token::Use)?;
        let mut path = vec![self.expect_ident()?];
        while self.eat(&Token::Dot) {
            path.push(self.expect_ident()?);
        }
        let alias = if self.eat(&Token::As) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(&Token::Semi)?;
        let span = start.merge(self.prev_span());
        Ok(UseDecl { path, alias, span })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Function declaration
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_fn_decl(&mut self) -> PResult<FnDecl> {
        let start = self.expect(&Token::Fn)?;

        // Optional name (lambdas have no name)
        let name = if !matches!(self.peek(), Some(Token::LParen)) {
            Some(self.parse_type_name()?)
        } else {
            None
        };

        // Parameters
        self.expect(&Token::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(&Token::RParen)?;

        // Return type and/or extern and/or body
        let mut return_type = None;
        let mut is_extern = false;
        let mut body = None;

        if !matches!(self.peek(), Some(Token::LBrace | Token::Semi) | None)
            && !matches!(self.peek(), Some(Token::Extern))
        {
            let ty = self.parse_type_name()?;
            // Check if the parsed "type" is actually the keyword `extern`
            if ty.as_name() == Some("extern") {
                is_extern = true;
            } else {
                return_type = Some(ty);
            }
        }

        if matches!(self.peek(), Some(Token::Extern)) {
            self.advance();
            is_extern = true;
        }

        if matches!(self.peek(), Some(Token::LBrace)) {
            body = Some(self.parse_block()?);
        } else {
            // Optional semicolon after extern
            self.eat(&Token::Semi);
        }

        // Support trailing parameter/allocator list: } (allocator = GlobalAllocator)
        let mut trailing_params = None;
        if matches!(self.peek(), Some(Token::LParen)) {
            self.advance();
            trailing_params = Some(self.parse_param_list()?);
            self.expect(&Token::RParen)?;
        }

        let span = start.merge(self.prev_span());
        Ok(FnDecl {
            name,
            params,
            return_type,
            body,
            is_extern,
            trailing_params,
            span,
        })
    }

    fn parse_param_list(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();
        while !matches!(self.peek(), Some(Token::RParen | Token::RBrace) | None) {
            let mutable = self.eat(&Token::Mut);
            // Skip 'ref' if it appears (seen in memory.ms)
            if let Some(Token::Ident(id)) = self.peek() {
                if id == "ref" {
                    self.advance();
                }
            }
            
            let name = self.expect_ident()?;
            
            // Support default values or ignore them for now: allocator = GlobalAllocator
            let mut ty = TypeExpr::Unknown;
            if !matches!(self.peek(), Some(Token::Eq | Token::Comma | Token::RParen | Token::RBrace)) {
                ty = self.parse_type_name()?;
            }
            
            if self.eat(&Token::Eq) {
                // For now just consume the expression as we don't store it in Param
                self.parse_expr(0)?;
            }

            let span = name.span.merge(self.prev_span());
            params.push(Param { name, mutable, ty, span });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(params)
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Type definition
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_type_def(&mut self) -> PResult<TypeDef> {
        let start = self.expect(&Token::Type)?;
        let name = self.parse_type_name()?;
        self.expect(&Token::Eq)?;

        let definition = match self.peek() {
            Some(Token::Struct) => {
                self.advance();
                TypeDefBody::Struct(self.parse_struct_def()?)
            }
            Some(Token::Enum) => {
                self.advance();
                TypeDefBody::Enum(self.parse_enum_def()?)
            }
            _ => TypeDefBody::Alias(self.parse_type_name()?),
        };

        self.eat(&Token::Semi);
        let span = start.merge(self.prev_span());
        Ok(TypeDef {
            name,
            definition,
            span,
        })
    }

    fn parse_struct_def(&mut self) -> PResult<StructDef> {
        let start = self.expect(&Token::LBrace)?;
        let fields = self.parse_param_list()?;
        let end = self.expect(&Token::RBrace)?;
        Ok(StructDef {
            fields,
            span: start.merge(end),
        })
    }

    fn parse_enum_def(&mut self) -> PResult<EnumDef> {
        let start = self.expect(&Token::LBrace)?;
        let mut variants = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            let name = self.expect_ident()?;
            let mut fields = Vec::new();
            if self.eat(&Token::LParen) {
                while !matches!(self.peek(), Some(Token::RParen) | None) {
                    fields.push(self.parse_type_name()?);
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RParen)?;
            }
            let vspan = name.span.merge(self.prev_span());
            variants.push(EnumVariant {
                name,
                fields,
                span: vspan,
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let end = self.expect(&Token::RBrace)?;
        Ok(EnumDef {
            variants,
            span: start.merge(end),
        })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Trait
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_trait(&mut self) -> PResult<TraitDef> {
        let start = self.expect(&Token::Trait)?;
        let name = self.parse_type_name()?;
        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        while matches!(self.peek(), Some(Token::Fn)) {
            methods.push(self.parse_fn_decl()?);
        }
        let end = self.expect(&Token::RBrace)?;
        Ok(TraitDef {
            name,
            methods,
            span: start.merge(end),
        })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Impl
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_impl(&mut self) -> PResult<ImplBlock> {
        let start = self.expect(&Token::Impl)?;

        // Optional generic params: impl[T]
        let mut generics = Vec::new();
        if self.eat(&Token::LBracket) {
            while !matches!(self.peek(), Some(Token::RBracket) | None) {
                generics.push(self.expect_ident()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::RBracket)?;
        }

        let trait_name = self.parse_type_name()?;

        // Optional `for Type`
        let for_type = if self.eat(&Token::For) {
            Some(self.parse_type_name()?)
        } else {
            None
        };

        self.expect(&Token::LBrace)?;
        let mut methods = Vec::new();
        while matches!(self.peek(), Some(Token::Fn)) {
            methods.push(self.parse_fn_decl()?);
        }
        let end = self.expect(&Token::RBrace)?;

        Ok(ImplBlock {
            generics,
            trait_name,
            for_type,
            methods,
            span: start.merge(end),
        })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Type name parsing
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_type_name(&mut self) -> PResult<TypeExpr> {
        // Handle reference prefix: @, @mut, &, &mut
        if matches!(self.peek(), Some(Token::At)) {
            self.advance();
            let mutable = self.eat(&Token::Mut);
            let inner = self.parse_type_name()?;
            return Ok(TypeExpr::Ref(Box::new(inner), mutable));
        }
        if matches!(self.peek(), Some(Token::Amp)) {
            self.advance();
            let mutable = self.eat(&Token::Mut);
            let inner = self.parse_type_name()?;
            return Ok(TypeExpr::Ref(Box::new(inner), mutable));
        }

        // Base: identifier
        let ident = if let Some(Token::CompilerFn(name)) = self.peek() {
            let span = self.peek_span();
            let name = format!("#{}", name);
            self.advance();
            Ident::new(&name, span)
        } else {
            self.expect_ident()?
        };
        let mut ty = TypeExpr::Named(ident);

        // Handle dot-separated nested types: std.net.IpAddr
        while self.eat(&Token::Dot) {
            if let Some(Token::Ident(_)) = self.peek() {
                // Only continue dot-path for type names if the next token is also an ident
                // and not followed by things that indicate it's an expression context
                let next_ident = self.expect_ident()?;
                ty = TypeExpr::Nested(Box::new(ty), Box::new(TypeExpr::Named(next_ident)));
            } else {
                // Put the dot back (we can't un-eat, so we decrement pos)
                self.pos -= 1;
                break;
            }
        }

        // Handle generics: Vec[T, U]
        if matches!(self.peek(), Some(Token::LBracket)) {
            self.advance();
            let mut params = Vec::new();
            while !matches!(self.peek(), Some(Token::RBracket) | None) {
                params.push(self.parse_type_name()?);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::RBracket)?;
            ty = TypeExpr::Generic(Box::new(ty), params);
        }

        Ok(ty)
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Block parsing
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_block(&mut self) -> PResult<Block> {
        let start = self.expect(&Token::LBrace)?;
        let mut items = Vec::new();

        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            items.push(self.parse_block_item()?);
        }

        let end = self.expect(&Token::RBrace)?;
        Ok(Block {
            items,
            span: start.merge(end),
        })
    }

    fn parse_block_item(&mut self) -> PResult<BlockItem> {
        match self.peek() {
            Some(Token::If) => Ok(BlockItem::IfChain(self.parse_if_chain()?)),
            Some(Token::Loop) => Ok(BlockItem::Loop(self.parse_loop()?)),
            Some(Token::Match) => Ok(BlockItem::Match(self.parse_match()?)),
            Some(Token::LBrace) => Ok(BlockItem::Block(self.parse_block()?)),
            _ => Ok(BlockItem::Statement(self.parse_statement()?)),
        }
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Statements
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_statement(&mut self) -> PResult<Statement> {
        match self.peek() {
            Some(Token::Let) | Some(Token::Mut) => self.parse_let_stmt(),
            Some(Token::Return) => self.parse_return_stmt(),
            Some(Token::Break) => self.parse_break_stmt(),
            Some(Token::Continue) => self.parse_continue_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_stmt(&mut self) -> PResult<Statement> {
        let start_span = self.peek_span();
        let mutable = matches!(self.peek(), Some(Token::Mut));
        self.advance(); // consume `let` or `mut`

        let name = self.expect_ident()?;

        // Optional type annotation (identifier that isn't `=`)
        let ty = if !matches!(self.peek(), Some(Token::Eq)) {
            self.eat(&Token::Colon);
            Some(self.parse_type_name()?)
        } else {
            None
        };

        self.expect(&Token::Eq)?;
        let value = self.parse_expr(0)?;
        self.expect(&Token::Semi)?;
        let span = start_span.merge(self.prev_span());

        Ok(Statement::Let {
            mutable,
            name,
            ty,
            value,
            span,
        })
    }

    fn parse_return_stmt(&mut self) -> PResult<Statement> {
        let start = self.expect(&Token::Return)?;
        let value = if matches!(self.peek(), Some(Token::Semi) | Some(Token::RBrace) | None) {
            None
        } else {
            Some(self.parse_expr(0)?)
        };
        self.expect(&Token::Semi)?;
        let span = start.merge(self.prev_span());
        Ok(Statement::Return { value, span })
    }

    fn parse_break_stmt(&mut self) -> PResult<Statement> {
        let start = self.expect(&Token::Break)?;
        let label = if let Some(Token::Ident(_)) = self.peek() {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(&Token::Semi)?;
        let span = start.merge(self.prev_span());
        Ok(Statement::Break { label, span })
    }

    fn parse_continue_stmt(&mut self) -> PResult<Statement> {
        let start = self.expect(&Token::Continue)?;
        let label = if let Some(Token::Ident(_)) = self.peek() {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(&Token::Semi)?;
        let span = start.merge(self.prev_span());
        Ok(Statement::Continue { label, span })
    }

    fn parse_expr_stmt(&mut self) -> PResult<Statement> {
        let expr = self.parse_expr(0)?;
        let span = expr.span();
        
        // Semicolon is optional if followed by RBrace or EOF
        if !matches!(self.peek(), Some(Token::RBrace) | None) {
            self.expect(&Token::Semi)?;
        } else {
            self.eat(&Token::Semi);
        }

        let span = span.merge(self.prev_span());
        Ok(Statement::Expr { expr, span })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  If / Elif / Else
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_if_chain(&mut self) -> PResult<IfChain> {
        let start = self.expect(&Token::If)?;
        let condition = self.parse_expr(0)?;
        let body = self.parse_block()?;
        let if_span = start.merge(self.prev_span());
        let if_block = ConditionalBlock {
            condition,
            body,
            span: if_span,
        };

        let mut elif_blocks = Vec::new();
        while matches!(self.peek(), Some(Token::Elif)) {
            let elif_start = self.advance().span;
            let condition = self.parse_expr(0)?;
            let body = self.parse_block()?;
            let span = elif_start.merge(self.prev_span());
            elif_blocks.push(ConditionalBlock {
                condition,
                body,
                span,
            });
        }

        let else_block = if self.eat(&Token::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };

        let span = start.merge(self.prev_span());
        Ok(IfChain {
            if_block,
            elif_blocks,
            else_block,
            span,
        })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Loop
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_loop(&mut self) -> PResult<LoopBlock> {
        let start = self.expect(&Token::Loop)?;
        let label = if let Some(Token::Ident(_)) = self.peek() {
            if matches!(self.peek_nth(1), Some(Token::LBrace)) {
                Some(self.expect_ident()?)
            } else {
                None
            }
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = start.merge(self.prev_span());
        Ok(LoopBlock { label, body, span })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Match
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    fn parse_match(&mut self) -> PResult<MatchBlock> {
        let start = self.expect(&Token::Match)?;
        let scrutinee = self.parse_expr(0)?;
        self.expect(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            let pattern = self.parse_expr(0)?;
            
            // Support optional Arrow (=>) or Colon (:)
            if !self.eat(&Token::Arrow) {
                self.eat(&Token::Colon);
            }

            let body = self.parse_block()?;
            self.eat(&Token::Comma);
            let span = pattern.span().merge(self.prev_span());
            arms.push(MatchArm {
                pattern,
                body,
                span,
            });
        }
        let end = self.expect(&Token::RBrace)?;
        Ok(MatchBlock {
            scrutinee,
            arms,
            span: start.merge(end),
        })
    }

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    //  Expression parsing — Pratt precedence climbing
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Pratt parser entry point. `min_bp` = minimum binding power.
    pub fn parse_expr(&mut self, min_bp: u8) -> PResult<Expr> {
        let mut lhs = self.parse_prefix()?;

        loop {
            if self.at_end() {
                break;
            }

            // Postfix operators
            if let Some(post_bp) = self.postfix_bp() {
                if post_bp < min_bp {
                    break;
                }
                lhs = self.parse_postfix(lhs)?;
                continue;
            }

            // Infix operators
            if let Some((l_bp, r_bp)) = self.infix_bp() {
                if l_bp < min_bp {
                    break;
                }

                // Special: `as` cast
                if matches!(self.peek(), Some(Token::As)) {
                    self.advance();
                    let ty = self.parse_type_name()?;
                    let span = lhs.span().merge(ty.span());
                    lhs = Expr::Cast {
                        expr: Box::new(lhs),
                        ty,
                        span,
                    };
                    continue;
                }

                // Special: `@=` pointer assign
                if matches!(self.peek(), Some(Token::AtAssign)) {
                    self.advance();
                    let rhs = self.parse_expr(r_bp)?;
                    let span = lhs.span().merge(rhs.span());
                    lhs = Expr::PointerAssign {
                        target: Box::new(lhs),
                        value: Box::new(rhs),
                        span,
                    };
                    continue;
                }

                // Special: `.` field access
                if matches!(self.peek(), Some(Token::Dot)) {
                    self.advance();
                    let field = self.expect_ident()?;
                    let span = lhs.span().merge(field.span);
                    lhs = Expr::Field {
                        object: Box::new(lhs),
                        field,
                        span,
                    };

                    // Check if this field access chain leads to a struct init
                    if self.is_struct_init_start() {
                        if let Some(ty) = Self::expr_to_type(&lhs) {
                            lhs = self.parse_struct_init_with_type(ty)?;
                        }
                    }
                    continue;
                }

                let op = self.parse_binop()?;
                let rhs = self.parse_expr(r_bp)?;
                let span = lhs.span().merge(rhs.span());
                lhs = Expr::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                    span,
                };
                continue;
            }

            break;
        }

        Ok(lhs)
    }

    /// Parse a prefix expression (unary or primary).
    fn parse_prefix(&mut self) -> PResult<Expr> {
        match self.peek().cloned() {
            // Unary prefix operators
            Some(Token::Minus) => {
                let span = self.advance().span;
                let operand = self.parse_expr(PREFIX_BP)?;
                let full_span = span.merge(operand.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span: full_span,
                })
            }
            Some(Token::At) => {
                let span = self.advance().span;
                let operand = self.parse_expr(PREFIX_BP)?;
                let full_span = span.merge(operand.span());
                Ok(Expr::Unary {
                    op: UnaryOp::AddrOf,
                    operand: Box::new(operand),
                    span: full_span,
                })
            }
            Some(Token::Star) => {
                let span = self.advance().span;
                let operand = self.parse_expr(PREFIX_BP)?;
                let full_span = span.merge(operand.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Deref,
                    operand: Box::new(operand),
                    span: full_span,
                })
            }

            // Parenthesized expression
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_expr(0)?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }

            // Array init: [a, b, c]
            Some(Token::LBracket) => self.parse_array_init(),

            // Lambda: fn (params) ret { body }
            Some(Token::Fn) => {
                let decl = self.parse_fn_decl()?;
                let span = decl.span;
                Ok(Expr::Lambda {
                    decl: Box::new(decl),
                    span,
                })
            }

            // Compiler function: #import(...)
            Some(Token::CompilerFn(name)) => {
                let start = self.advance().span;
                let name = name;
                self.expect(&Token::LParen)?;
                let args = self.parse_expr_list(&Token::RParen)?;
                let end = self.expect(&Token::RParen)?;
                Ok(Expr::CompilerCall {
                    name,
                    args,
                    span: start.merge(end),
                })
            }

            // Literals
            Some(Token::Int(v)) => {
                let span = self.advance().span;
                Ok(Expr::IntLit { value: v, span })
            }
            Some(Token::Float(v)) => {
                let span = self.advance().span;
                Ok(Expr::FloatLit { value: v, span })
            }
            Some(Token::String(ref v)) => {
                let v = v.clone();
                let span = self.advance().span;
                Ok(Expr::StringLit { value: v, span })
            }
            Some(Token::Char(v)) => {
                let span = self.advance().span;
                Ok(Expr::CharLit { value: v, span })
            }
            Some(Token::Bool(v)) => {
                let span = self.advance().span;
                Ok(Expr::BoolLit { value: v, span })
            }

            // Identifier — may be followed by struct init `Ident { ... }`
            Some(Token::Ident(_)) => {
                let ident = self.expect_ident()?;

                // Check for struct initialization: TypeName { field: val, ... }
                // We need to distinguish struct init from a block after an expression.
                // Heuristic: if next is `{` and the token after is `word =` or `word :`,
                // treat it as struct init.
                if matches!(self.peek(), Some(Token::LBracket)) {
                    // Could be generic type: Foo[T] { ... }
                    let saved = self.pos;
                    self.advance(); // consume [
                    if let Ok(params) = self.try_parse_generic_args() {
                        let ty = TypeExpr::Generic(
                            Box::new(TypeExpr::Named(ident.clone())),
                            params,
                        );
                        if self.is_struct_init_start() {
                            return self.parse_struct_init_with_type(ty);
                        }
                        // Not struct init — this is a generic type used as an expression
                        return Ok(Expr::TypeExpr(ty));
                    } else {
                        self.pos = saved;
                    }
                }

                if self.is_struct_init_start() {
                    let ty = TypeExpr::Named(ident);
                    return self.parse_struct_init_with_type(ty);
                }

                Ok(Expr::Ident(ident))
            }

            other => Err(self.error(format!("expected expression, found {:?}", other))),
        }
    }

    /// Try to convert an expression (field-access chain) back into a TypeExpr.
    /// Used when we discover that what looked like field access is actually a type name
    /// for struct initialization, e.g., `mantis.SysCallSignature { ... }`.
    fn expr_to_type(expr: &Expr) -> Option<TypeExpr> {
        match expr {
            Expr::Ident(id) => Some(TypeExpr::Named(id.clone())),
            Expr::Field { object, field, .. } => {
                let base = Self::expr_to_type(object)?;
                Some(TypeExpr::Nested(
                    Box::new(base),
                    Box::new(TypeExpr::Named(field.clone())),
                ))
            }
            Expr::TypeExpr(ty) => Some(ty.clone()),
            _ => None,
        }
    }

    fn try_parse_generic_args(&mut self) -> PResult<Vec<TypeExpr>> {
        let mut params = Vec::new();
        while !matches!(self.peek(), Some(Token::RBracket) | None) {
            params.push(self.parse_type_name()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RBracket)?;
        Ok(params)
    }

    fn is_struct_init_start(&self) -> bool {
        if !matches!(self.peek(), Some(Token::LBrace)) {
            return false;
        }
        // Look ahead: { ident = ... } or { ident : ... }
        if let Some(Token::Ident(_)) = self.peek_nth(1) {
            matches!(
                self.peek_nth(2),
                Some(Token::Eq) | Some(Token::Colon)
            )
        } else {
            false
        }
    }

    fn parse_struct_init_with_type(&mut self, ty: TypeExpr) -> PResult<Expr> {
        let start = ty.span();
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            let name = self.expect_ident()?;
            // Accept both `=` and `:` as field separators
            if !self.eat(&Token::Eq) {
                self.expect(&Token::Colon)?;
            }
            let value = self.parse_expr(0)?;
            let fspan = name.span.merge(value.span());
            fields.push(FieldInit {
                name,
                value,
                span: fspan,
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let end = self.expect(&Token::RBrace)?;
        Ok(Expr::StructInit {
            ty,
            fields,
            span: start.merge(end),
        })
    }

    fn parse_array_init(&mut self) -> PResult<Expr> {
        let start = self.expect(&Token::LBracket)?;
        let elements = self.parse_expr_list(&Token::RBracket)?;
        let end = self.expect(&Token::RBracket)?;
        Ok(Expr::ArrayInit {
            elements,
            span: start.merge(end),
        })
    }

    fn parse_expr_list(&mut self, terminator: &Token) -> PResult<Vec<Expr>> {
        let mut list = Vec::new();
        while !matches!(self.peek(), tok if tok.map_or(true, |t| std::mem::discriminant(t) == std::mem::discriminant(terminator)))
        {
            list.push(self.parse_expr(0)?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(list)
    }

    /// Parse postfix: function call `(...)`, propagate `?`
    fn parse_postfix(&mut self, lhs: Expr) -> PResult<Expr> {
        match self.peek() {
            Some(Token::LParen) => {
                self.advance();
                let args = self.parse_expr_list(&Token::RParen)?;
                let end = self.expect(&Token::RParen)?;
                let span = lhs.span().merge(end);
                Ok(Expr::Call {
                    callee: Box::new(lhs),
                    args,
                    span,
                })
            }
            Some(Token::Question) => {
                let end = self.advance().span;
                let span = lhs.span().merge(end);
                Ok(Expr::Propagate {
                    expr: Box::new(lhs),
                    span,
                })
            }
            _ => Err(self.error("expected postfix operator".into())),
        }
    }

    fn parse_binop(&mut self) -> PResult<BinOp> {
        let op = match self.peek() {
            Some(Token::Plus) => BinOp::Add,
            Some(Token::Minus) => BinOp::Sub,
            Some(Token::Star) => BinOp::Mul,
            Some(Token::Slash) => BinOp::Div,
            Some(Token::Percent) => BinOp::Mod,
            Some(Token::EqEq) => BinOp::Eq,
            Some(Token::NotEq) => BinOp::NotEq,
            Some(Token::Gt) => BinOp::Gt,
            Some(Token::Lt) => BinOp::Lt,
            Some(Token::GtEq) => BinOp::GtEq,
            Some(Token::LtEq) => BinOp::LtEq,
            Some(Token::Eq) => BinOp::Assign,
            Some(Token::Shr) => BinOp::Shr,
            Some(Token::Shl) => BinOp::Shl,
            Some(Token::Amp) => BinOp::BitAnd,
            Some(Token::Pipe) => BinOp::BitOr,
            Some(Token::Caret) => BinOp::BitXor,
            _ => return Err(self.error(format!("expected binary operator, found {:?}", self.peek()))),
        };
        self.advance();
        Ok(op)
    }

    fn infix_bp(&self) -> Option<(u8, u8)> {
        match self.peek()? {
            Token::Eq | Token::AtAssign => Some((2, 1)),    // right-assoc assignment
            Token::EqEq | Token::NotEq => Some((3, 4)),
            Token::Gt | Token::Lt | Token::GtEq | Token::LtEq => Some((5, 6)),
            Token::Pipe => Some((7, 8)),
            Token::Caret => Some((9, 10)),
            Token::Amp => Some((11, 12)),
            Token::Shl | Token::Shr => Some((13, 14)),
            Token::Plus | Token::Minus => Some((15, 16)),
            Token::Star | Token::Slash | Token::Percent => Some((17, 18)),
            Token::As => Some((19, 20)),                     // cast
            Token::Dot => Some((25, 26)),                    // field access
            _ => None,
        }
    }

    fn postfix_bp(&self) -> Option<u8> {
        match self.peek()? {
            Token::LParen => Some(25),  // function call
            Token::Question => Some(25), // propagate
            _ => None,
        }
    }
}

/// Binding power for prefix operators.
const PREFIX_BP: u8 = 23;
