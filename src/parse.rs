use crate::ast::{Call, Expr, Stmt, StmtKind, Var};

pub type It<'a> = std::iter::Peekable<std::str::Chars<'a>>;

fn is_ident_char(c: char) -> bool {
    matches!(c, '!'..'z') && !matches!(c, '(' | ')')
}

pub trait Parse: Sized {
    fn parse_it(it: &mut It) -> Option<Self>;
    fn must_parse(s: &str) -> Self {
        let mut it = s.chars().peekable();
        let s = Self::parse_it(&mut it).unwrap();
        assert!(it.next().is_none());
        s
    }
}

impl Parse for u32 {
    fn parse_it(it: &mut It) -> Option<Self> {
        let mut buf = String::new();
        while let Some(c @ '0'..'9') = it.peek() {
            buf.push(*c);
            it.next();
        }
        assert!(!buf.is_empty());
        Some(<u32>::from_str_radix(&buf, 16).unwrap())
    }
}

impl Parse for Var {
    fn parse_it(it: &mut It) -> Option<Self> {
        let mut buf = String::new();
        while let Some(&c) = it.peek() {
            if !is_ident_char(c) {
                break;
            }
            buf.push(c);
            it.next();
        }
        assert!(!buf.is_empty());
        Some(Var::new(buf))
    }
}

impl Parse for Call {
    fn parse_it(it: &mut It) -> Option<Self> {
        let '(' = it.next()? else {
            unreachable!();
        };

        let mut args = vec![];
        loop {
            if *it.peek()? == ')' {
                it.next();
                break;
            }
            args.push(<Expr>::parse_it(it).unwrap());
            while *it.peek()? == ' ' {
                it.next();
            }
        }

        let Expr::Var(func) = args.remove(0) else {
            panic!()
        };
        Some(Call {
            func: func.to_string(),
            args,
        })
    }
}

impl Parse for Expr {
    fn parse_it(it: &mut It) -> Option<Self> {
        Some(match *it.peek()? {
            '0'..'9' => Expr::Val(<u32>::parse_it(it)?),
            '(' => Expr::Call(Box::new(<Call>::parse_it(it)?)),
            c if is_ident_char(c) => Expr::Var(<Var>::parse_it(it)?),
            _ => return None,
        })
    }
}

impl Parse for StmtKind {
    fn parse_it(it: &mut It) -> Option<Self> {
        let expr = <Expr>::parse_it(it)?;
        let Expr::Call(call) = &expr else { panic!() };
        let Call { func, args } = &**call;
        let stmt = match func.as_str() {
            "set" => {
                let [a, b] = args.clone().try_into().unwrap();
                StmtKind::Set(a, b)
            }
            _ => StmtKind::Do(expr),
        };
        Some(stmt)
    }
}

impl Parse for Stmt {
    fn parse_it(it: &mut It) -> Option<Self> {
        let kind = <StmtKind>::parse_it(it)?;
        Some(Stmt { ip: vec![], kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: Parse + std::fmt::Display>(text: &str) -> String {
        format!("{}", <T>::must_parse(text))
    }

    #[test]
    fn expr() {
        insta::assert_snapshot!(parse::<Expr>("12"), @"0x12");
        insta::assert_snapshot!(parse::<Expr>("foo"), @"foo");
        insta::assert_snapshot!(parse::<Expr>("(foo bar)"), @"(foo bar)");
        insta::assert_snapshot!(parse::<Expr>("(+ 1 2)"), @"(+ 1 2)");
    }

    #[test]
    fn stmt() {
        insta::assert_snapshot!(parse::<Stmt>("(set x 3)"), @"(set x 3)");
    }
}
