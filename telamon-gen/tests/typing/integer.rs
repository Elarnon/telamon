pub use super::utils::RcStr;

pub use super::telamon_gen::lexer::{Lexer, Spanned, Position};
pub use super::telamon_gen::parser;
pub use super::telamon_gen::ast::*;

#[cfg(test)]
mod undefined {
    pub use super::*;

    /// Missing the set MySet from a Integer.
    #[test]
    fn parameter() {
        assert_eq!(parser::parse_ast(Lexer::from(
            b"define integer foo($arg in MySet): \"mycode\"
              end".to_vec())).unwrap().type_check().err(),
            Some(TypeError::Undefined(Spanned {
                beg: Position { line: 0, column: 0},
                end: Position { line: 1, column: 17},
                data: String::from("MySet"),
            }))
        );
    }
}

#[cfg(test)]
mod redefinition {
    pub use super::*;

    /// Redefinition of the foo Integer.
    #[test]
    fn integer() {
        assert_eq!(parser::parse_ast(Lexer::from(
            b"set Arg:
                item_type = \"ir::inst::Obj\"
                id_type = \"ir::inst::Id\"
                item_getter = \"ir::inst::get($fun, $id)\"
                id_getter = \"ir::inst::Obj::id($item)\"
                iterator = \"ir::inst::iter($fun)\"
                var_prefix = \"inst\"
                new_objs = \"$objs.inst\"
              end
              define integer foo($myarg in Arg): \"mycode\"
              end
              define integer foo($myarg in Arg): \"mycode\"
              end".to_vec())).unwrap().type_check().err(),
            Some(TypeError::Redefinition(Spanned {
                beg: Position { line: 9, column: 29},
                end: Position { line: 9, column: 32},
                data: Hint::Integer,
            }, Spanned {
                beg: Position { line: 11, column: 29},
                end: Position { line: 11, column: 32},
                data:  String::from("foo"),
            }))
        );
    }
}
