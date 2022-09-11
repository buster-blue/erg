use erg_common::dict::Dict;
use erg_common::log;
use erg_common::traits::{Locational, Stream};
use erg_common::Str;

use erg_parser::ast::{ClassDef, Expr, Module, PreDeclTypeSpec, TypeSpec, AST};

use crate::error::{TyCheckError, TyCheckErrors};

/// Combine method definitions across multiple modules, specialized class contexts, etc.
#[derive(Debug, Default)]
pub struct Linker {
    // TODO: inner scope types
    pub def_root_pos_map: Dict<Str, usize>,
    pub errs: TyCheckErrors,
}

impl Linker {
    pub fn new() -> Self {
        Self {
            def_root_pos_map: Dict::new(),
            errs: TyCheckErrors::empty(),
        }
    }

    pub fn link(mut self, mut ast: AST) -> Result<AST, TyCheckErrors> {
        log!(info "the linking process has started.");
        let mut new = vec![];
        while let Some(chunk) = ast.module.lpop() {
            match chunk {
                Expr::Def(def) => {
                    match def.body.block.first().unwrap() {
                        Expr::Call(call) => {
                            match call.obj.get_name().map(|s| &s[..]) {
                                // TODO: decorator
                                Some("Class" | "Inherit" | "Inheritable") => {
                                    self.def_root_pos_map.insert(
                                        def.sig.ident().unwrap().inspect().clone(),
                                        new.len(),
                                    );
                                    let type_def = ClassDef::new(def, vec![]);
                                    new.push(Expr::ClassDef(type_def));
                                }
                                _ => {
                                    new.push(Expr::Def(def));
                                }
                            }
                        }
                        _ => {
                            new.push(Expr::Def(def));
                        }
                    }
                }
                Expr::Methods(methods) => match &methods.class {
                    TypeSpec::PreDeclTy(PreDeclTypeSpec::Simple(simple)) => {
                        if let Some(pos) = self.def_root_pos_map.get(simple.name.inspect()) {
                            let mut class_def = match new.remove(*pos) {
                                Expr::ClassDef(class_def) => class_def,
                                _ => unreachable!(),
                            };
                            class_def.methods_list.push(methods);
                            new.insert(*pos, Expr::ClassDef(class_def));
                        } else {
                            let similar_name = Str::from(
                                self.def_root_pos_map
                                    .keys()
                                    .fold("".to_string(), |acc, key| acc + &key[..] + ","),
                            );
                            self.errs.push(TyCheckError::no_var_error(
                                line!() as usize,
                                methods.class.loc(),
                                "".into(),
                                simple.name.inspect(),
                                Some(&similar_name),
                            ));
                        }
                    }
                    other => todo!("{other}"),
                },
                other => {
                    new.push(other);
                }
            }
        }
        let ast = AST::new(ast.name, Module::new(new));
        log!(info "the linking process has completed:\n{}", ast);
        if self.errs.is_empty() {
            Ok(ast)
        } else {
            Err(self.errs)
        }
    }
}