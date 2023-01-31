use erg_common::config::Input;
use erg_common::error::{ErrorCore, ErrorKind::*, Location, SubMessage};
use erg_common::style::{StyledStr, StyledString, StyledStrings};
use erg_common::traits::Locational;
use erg_common::vis::Visibility;
use erg_common::{switch_lang, Str};

use crate::error::*;
use crate::hir::{Expr, Identifier};
use crate::ty::{HasType, Type};

pub type LowerError = CompileError;
pub type LowerWarning = LowerError;
pub type LowerErrors = CompileErrors;
pub type LowerWarnings = LowerErrors;
pub type LowerResult<T> = CompileResult<T>;
pub type SingleLowerResult<T> = SingleCompileResult<T>;

impl LowerError {
    pub fn syntax_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        desc: String,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                desc,
                errno,
                SyntaxError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn unused_expr_warning(input: Input, errno: usize, expr: &Expr, caused_by: String) -> Self {
        let desc = switch_lang!(
            "japanese" => format!("式の評価結果(: {})が使われていません", expr.ref_t()),
            "simplified_chinese" => format!("表达式评估结果(: {})未使用", expr.ref_t()),
            "traditional_chinese" => format!("表達式評估結果(: {})未使用", expr.ref_t()),
            "english" => format!("the evaluation result of the expression (: {}) is not used", expr.ref_t()),
        );
        let discard = StyledString::new("discard", Some(HINT), Some(ATTR));
        let hint = switch_lang!(
            "japanese" => format!("値を使わない場合は、{discard}関数を使用してください"),
            "simplified_chinese" => format!("如果您不想使用该值，请使用{discard}函数"),
            "traditional_chinese" => format!("如果您不想使用該值，請使用{discard}函數"),
            "english" => format!("if you don't use the value, use {discard} function"),
        );
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(expr.loc(), vec![], Some(hint))],
                desc,
                errno,
                UnusedWarning,
                expr.loc(),
            ),
            input,
            caused_by,
        )
    }

    pub fn duplicate_decl_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
    ) -> Self {
        let name = readable_name(name);
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("{name}は既に宣言されています"),
                    "simplified_chinese" => format!("{name}已声明"),
                    "traditional_chinese" => format!("{name}已聲明"),
                    "english" => format!("{name} is already declared"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn duplicate_definition_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
    ) -> Self {
        let name = readable_name(name);
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("{name}は既に定義されています"),
                    "simplified_chinese" => format!("{name}已定义"),
                    "traditional_chinese" => format!("{name}已定義"),
                    "english" => format!("{name} is already defined"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn violate_decl_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        spec_t: &Type,
        found_t: &Type,
    ) -> Self {
        let name = StyledString::new(readable_name(name), Some(WARN), None);
        let expect = StyledString::new(format!("{spec_t}"), Some(HINT), Some(ATTR));
        let found = StyledString::new(format!("{found_t}"), Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("{name}は{expect}型として宣言されましたが、{found}型のオブジェクトが代入されています"),
                    "simplified_chinese" => format!("{name}被声明为{expect}，但分配了一个{found}对象"),
                    "traditional_chinese" => format!("{name}被聲明為{expect}，但分配了一個{found}對象"),
                    "english" => format!("{name} was declared as {expect}, but an {found} object is assigned"),
                ),
                errno,
                TypeError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn no_var_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        similar_name: Option<&str>,
    ) -> Self {
        let name = readable_name(name);
        let hint = similar_name.map(|n| {
            let n = StyledStr::new(n, Some(HINT), Some(ATTR));
            switch_lang!(
                "japanese" => format!("似た名前の変数があります: {n}"),
                "simplified_chinese" => format!("存在相同名称变量: {n}"),
                "traditional_chinese" => format!("存在相同名稱變量: {n}"),
                "english" => format!("exists a similar name variable: {n}"),
            )
        });
        let found = StyledString::new(name, Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("{found}という変数は定義されていません"),
                    "simplified_chinese" => format!("{found}未定义"),
                    "traditional_chinese" => format!("{found}未定義"),
                    "english" => format!("{found} is not defined"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn access_before_def_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        defined_line: u32,
        similar_name: Option<&str>,
    ) -> Self {
        let name = readable_name(name);
        let hint = similar_name.map(|n| {
            let n = StyledStr::new(n, Some(HINT), Some(ATTR));
            switch_lang!(
                "japanese" => format!("似た名前の変数があります: {n}"),
                "simplified_chinese" => format!("存在相同名称变量: {n}"),
                "traditional_chinese" => format!("存在相同名稱變量: {n}"),
                "english" => format!("exists a similar name variable: {n}"),
            )
        });
        let found = StyledString::new(name, Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("定義({defined_line}行目)より前で{found}を参照することは出来ません"),
                    "simplified_chinese" => format!("在{found}定义({defined_line}行)之前引用是不允许的"),
                    "traditional_chinese" => format!("在{found}定義({defined_line}行)之前引用是不允許的"),
                    "english" => format!("cannot access {found} before its definition (line {defined_line})"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn access_deleted_var_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        del_line: u32,
        similar_name: Option<&str>,
    ) -> Self {
        let name = readable_name(name);
        let hint = similar_name.map(|n| {
            let n = StyledStr::new(n, Some(HINT), Some(ATTR));
            switch_lang!(
                "japanese" => format!("似た名前の変数があります: {n}"),
                "simplified_chinese" => format!("存在相同名称变量: {n}"),
                "traditional_chinese" => format!("存在相同名稱變量: {n}"),
                "english" => format!("exists a similar name variable: {n}"),
            )
        });
        let found = StyledString::new(name, Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("削除された変数{found}を参照することは出来ません({del_line}行目で削除)"),
                    "simplified_chinese" => format!("不能引用已删除的变量{found}({del_line}行)"),
                    "traditional_chinese" => format!("不能引用已刪除的變量{found}({del_line}行)"),
                    "english" => format!("cannot access deleted variable {found} (deleted at line {del_line})"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn no_type_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        similar_name: Option<&str>,
    ) -> Self {
        let name = readable_name(name);
        let hint = similar_name.map(|n| {
            let n = StyledStr::new(n, Some(HINT), Some(ATTR));
            switch_lang!(
                "japanese" => format!("似た名前の型があります: {n}"),
                "simplified_chinese" => format!("存在相同名称类型: {n}"),
                "traditional_chinese" => format!("存在相同名稱類型: {n}"),
                "english" => format!("exists a similar name type: {n}"),
            )
        });
        let found = StyledString::new(name, Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("{found}という型は定義されていません"),
                    "simplified_chinese" => format!("{found}未定义"),
                    "traditional_chinese" => format!("{found}未定義"),
                    "english" => format!("Type {found} is not defined"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn type_not_found(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        typ: &Type,
    ) -> Self {
        let typ = StyledString::new(typ.to_string(), Some(ERR), Some(ATTR));
        let hint = Some(switch_lang!(
            "japanese" => format!("恐らくこれはErgコンパイラのバグです、{URL}へ報告してください"),
            "simplified_chinese" => format!("这可能是Erg编译器的错误，请报告给{URL}"),
            "traditional_chinese" => format!("這可能是Erg編譯器的錯誤，請報告給{URL}"),
            "english" => format!("This may be a bug of Erg compiler, please report to {URL}"),
        ));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("{typ}という型は定義されていません"),
                    "simplified_chinese" => format!("{typ}未定义"),
                    "traditional_chinese" => format!("{typ}未定義"),
                    "english" => format!("Type {typ} is not defined"),
                ),
                errno,
                NameError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn no_attr_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        obj_t: &Type,
        name: &str,
        similar_name: Option<&str>,
    ) -> Self {
        let hint = similar_name.map(|n| {
            switch_lang!(
                "japanese" => format!("似た名前の属性があります: {n}"),
                "simplified_chinese" => format!("具有相同名称的属性: {n}"),
                "traditional_chinese" => format!("具有相同名稱的屬性: {n}"),
                "english" => format!("has a similar name attribute: {n}"),
            )
        });
        let found = StyledString::new(name, Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("{obj_t}型オブジェクトに{found}という属性はありません"),
                    "simplified_chinese" => format!("{obj_t}对象没有属性{found}"),
                    "traditional_chinese" => format!("{obj_t}對像沒有屬性{found}"),
                    "english" => format!("{obj_t} object has no attribute {found}"),
                ),
                errno,
                AttributeError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn singular_no_attr_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        obj_name: &str,
        obj_t: &Type,
        name: &str,
        similar_name: Option<&str>,
    ) -> Self {
        let hint = similar_name.map(|n| {
            let n = StyledStr::new(n, Some(HINT), Some(ATTR));
            switch_lang!(
                "japanese" => format!("似た名前の属性があります: {n}"),
                "simplified_chinese" => format!("具有相同名称的属性: {n}"),
                "traditional_chinese" => format!("具有相同名稱的屬性: {n}"),
                "english" => format!("has a similar name attribute: {n}"),
            )
        });
        let found = StyledString::new(name, Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("{obj_name}(: {obj_t})に{found}という属性はありません"),
                    "simplified_chinese" => format!("{obj_name}(: {obj_t})没有属性{found}"),
                    "traditional_chinese" => format!("{obj_name}(: {obj_t})沒有屬性{found}"),
                    "english" => format!("{obj_name}(: {obj_t}) has no attribute {found}"),
                ),
                errno,
                AttributeError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn reassign_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
    ) -> Self {
        let name = StyledStr::new(readable_name(name), Some(WARN), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("変数{name}に複数回代入することはできません"),
                    "simplified_chinese" => format!("不能为变量{name}分配多次"),
                    "traditional_chinese" => format!("不能為變量{name}分配多次"),
                    "english" => format!("variable {name} cannot be assigned more than once"),
                ),
                errno,
                AssignError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn del_error(
        input: Input,
        errno: usize,
        ident: &Identifier,
        is_const: bool,
        caused_by: String,
    ) -> Self {
        let prefix = if is_const {
            switch_lang!(
                "japanese" => "定数",
                "simplified_chinese" => "定数",
                "traditional_chinese" => "定數",
                "english" => "constant",
            )
        } else {
            switch_lang!(
                "japanese" => "組み込み変数",
                "simplified_chinese" => "内置变量",
                "traditional_chinese" => "内置變量",
                "english" => "built-in variable",
            )
        };
        let name = StyledString::new(readable_name(ident.inspect()), Some(WARN), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(ident.loc())],
                switch_lang!(
                    "japanese" => format!("{prefix}{name}は削除できません"),
                    "simplified_chinese" => format!("{prefix}{name}不能删除"),
                    "traditional_chinese" => format!("{prefix}{name}不能刪除"),
                    "english" => format!("{prefix} {name} cannot be deleted"),
                ),
                errno,
                NameError,
                ident.loc(),
            ),
            input,
            caused_by,
        )
    }

    pub fn visibility_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        vis: Visibility,
    ) -> Self {
        let visibility = if vis.is_private() {
            switch_lang!(
                "japanese" => "非公開",
                "simplified_chinese" => "私有",
                "traditional_chinese" => "私有",
                "english" => "private",
            )
        } else {
            switch_lang!(
                "japanese" => "公開",
                "simplified_chinese" => "公有",
                "traditional_chinese" => "公有",
                "english" => "public",
            )
        };
        let found = StyledString::new(readable_name(name), Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("{found}は{visibility}変数です"),
                    "simplified_chinese" => format!("{found}是{visibility}变量",),
                    "traditional_chinese" => format!("{found}是{visibility}變量",),
                    "english" => format!("{found} is {visibility} variable",),
                ),
                errno,
                VisibilityError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn override_error<S: Into<String>>(
        input: Input,
        errno: usize,
        name: &str,
        name_loc: Location,
        superclass: &Type,
        caused_by: S,
    ) -> Self {
        let name = StyledString::new(name, Some(ERR), Some(ATTR));
        let superclass = StyledString::new(format!("{superclass}"), Some(WARN), Some(ATTR));
        let hint = Some(
            switch_lang!(
                "japanese" => {
                    let mut ovr = StyledStrings::default();
                    ovr.push_str_with_color_and_attribute("@Override", HINT, ATTR);
                    ovr.push_str("デコレータを使用してください");
                    ovr
            },
                "simplified_chinese" => {
                    let mut ovr = StyledStrings::default();
                    ovr.push_str("请使用");
                    ovr.push_str_with_color_and_attribute("@Override", HINT, ATTR);
                    ovr.push_str("装饰器");
                    ovr
                },
                "traditional_chinese" => {
                    let mut ovr = StyledStrings::default();
                    ovr.push_str("請使用");
                    ovr.push_str_with_color_and_attribute("@Override", HINT, ATTR);
                    ovr.push_str("裝飾器");
                    ovr
                },
                "english" => {
                    let mut ovr = StyledStrings::default();
                    ovr.push_str("use ");
                    ovr.push_str_with_color_and_attribute("@Override", HINT, ATTR);
                    ovr.push_str(" decorator");
                    ovr
                },
            )
            .to_string(),
        );
        let sub_msg = switch_lang!(
            "japanese" => "デフォルトでオーバーライドはできません",
            "simplified_chinese" => "默认不可重写",
            "simplified_chinese" => "默認不可重寫",
            "english" => "cannot override by default",
        );
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(
                    name_loc,
                    vec![sub_msg.to_string()],
                    hint,
                )],
                switch_lang!(
                    "japanese" => format!(
                        "{name}は{superclass}で既に定義されています",
                    ),
                    "simplified_chinese" => format!(
                        "{name}已在{superclass}中定义",
                    ),
                    "traditional_chinese" => format!(
                        "{name}已在{superclass}中定義",
                    ),
                    "english" => format!(
                        "{name} is already defined in {superclass}",
                    ),
                ),
                errno,
                NameError,
                name_loc,
            ),
            input,
            caused_by.into(),
        )
    }

    pub fn inheritance_error(
        input: Input,
        errno: usize,
        class: String,
        loc: Location,
        caused_by: String,
    ) -> Self {
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("{class}は継承できません"),
                    "simplified_chinese" => format!("{class}不可继承"),
                    "traditional_chinese" => format!("{class}不可繼承"),
                    "english" => format!("{class} is not inheritable"),
                ),
                errno,
                InheritanceError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn file_error(
        input: Input,
        errno: usize,
        desc: String,
        loc: Location,
        caused_by: String,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                desc,
                errno,
                IoError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn module_env_error(
        input: Input,
        errno: usize,
        mod_name: &str,
        loc: Location,
        caused_by: String,
    ) -> Self {
        let desc = switch_lang!(
            "japanese" => format!("{mod_name}モジュールはお使いの環境をサポートしていません"),
            "simplified_chinese" => format!("{mod_name}模块不支持您的环境"),
            "traditional_chinese" => format!("{mod_name}模塊不支持您的環境"),
            "english" => format!("module {mod_name} is not supported in your environment"),
        );
        Self::file_error(input, errno, desc, loc, caused_by, None)
    }

    pub fn import_error(
        input: Input,
        errno: usize,
        desc: String,
        loc: Location,
        caused_by: String,
        similar_erg_mod: Option<Str>,
        similar_py_mod: Option<Str>,
    ) -> Self {
        let mut erg_str = StyledStrings::default();
        let mut py_str = StyledStrings::default();
        let hint = switch_lang!(
        "japanese" => {
            match (similar_erg_mod, similar_py_mod) {
                (Some(erg), Some(py)) => {
                    erg_str.push_str("似た名前のergモジュールが存在します: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    py_str.push_str("似た名前のpythonモジュールが存在します: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("pythonのモジュールをインポートするためには");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    hint.push_str("を使用してください");
                    Some(hint.to_string())
                }
                (Some(erg), None) => {
                    erg_str.push_str("似た名前のergモジュールが存在します");
                    erg_str.push_str_with_color_and_attribute(erg, ACCENT, ATTR);
                    None
                }
                (None, Some(py)) => {
                    py_str.push_str("似た名前のpythonモジュールが存在します");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("pythonのモジュールをインポートするためには");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    hint.push_str("を使用してください");
                    Some(hint.to_string())
                }
                (None, None) => None,
            }
        },
        "simplified_chinese" => {
            match (similar_erg_mod, similar_py_mod) {
                (Some(erg), Some(py)) => {
                    erg_str.push_str("存在相似名称的erg模块: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    py_str.push_str("存在相似名称的python模块: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("要导入python模块,请使用");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    Some(hint.to_string())
                }
                (Some(erg), None) => {
                    erg_str.push_str("存在相似名称的erg模块: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    None
                }
                (None, Some(py)) => {
                    py_str.push_str("存在相似名称的python模块: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("要导入python模块,请使用");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    Some(hint.to_string())
                }
                (None, None) => None,
            }
        },
        "traditional_chinese" => {
            match (similar_erg_mod, similar_py_mod) {
                (Some(erg), Some(py)) => {
                    erg_str.push_str("存在類似名稱的erg模塊: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    py_str.push_str("存在類似名稱的python模塊: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("要導入python模塊, 請使用");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    Some(hint.to_string())
                }
                (Some(erg), None) => {
                    erg_str.push_str("存在類似名稱的erg模塊: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    None
                }
                (None, Some(py)) => {
                    py_str.push_str("存在類似名稱的python模塊: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("要導入python模塊, 請使用");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    Some(hint.to_string())
                }
                (None, None) => None,
            }
        },
        "english" => {
            match (similar_erg_mod, similar_py_mod) {
                (Some(erg), Some(py)) => {
                    erg_str.push_str("similar name erg module exists: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    py_str.push_str("similar name python module exists: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("to import python modules, use ");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    Some(hint.to_string())
                }
                (Some(erg), None) => {
                    erg_str.push_str("similar name erg module exists: ");
                    erg_str.push_str_with_color_and_attribute(erg, HINT, ATTR);
                    None
                }
                (None, Some(py)) => {
                    py_str.push_str("similar name python module exits: ");
                    py_str.push_str_with_color_and_attribute(py, HINT, ATTR);
                    let mut hint  = StyledStrings::default();
                    hint.push_str("to import python modules, use ");
                    hint.push_str_with_color_and_attribute("pyimport", ACCENT, ATTR);
                    Some(hint.to_string())
                }
                (None, None) => None,
            }
        },
        );
        // .to_string().is_empty() is not necessarily empty because there are Color or Attribute that are not displayed
        let msg = match (erg_str.is_empty(), py_str.is_empty()) {
            (false, false) => vec![erg_str.to_string(), py_str.to_string()],
            (false, true) => vec![erg_str.to_string()],
            (true, false) => vec![py_str.to_string()],
            (true, true) => vec![],
        };
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, msg, hint)],
                desc,
                errno,
                ImportError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn inner_typedef_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
    ) -> Self {
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("型はトップレベルで定義されなければなりません"),
                    "simplified_chinese" => format!("类型必须在顶层定义"),
                    "traditional_chinese" => format!("類型必須在頂層定義"),
                    "english" => format!("types must be defined at the top level"),
                ),
                errno,
                TypeError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn declare_error(input: Input, errno: usize, loc: Location, caused_by: String) -> Self {
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("d.erファイル内では宣言、別名定義のみが許可されています"),
                    "simplified_chinese" => format!("在d.er文件中只允许声明和别名定义"),
                    "traditional_chinese" => format!("在d.er文件中只允許聲明和別名定義"),
                    "english" => format!("declarations and alias definitions are only allowed in d.er files"),
                ),
                errno,
                SyntaxError,
                loc,
            ),
            input,
            caused_by,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn invalid_type_cast_error(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
        cast_to: &Type,
        hint: Option<String>,
    ) -> Self {
        let name = StyledString::new(name, Some(WARN), Some(ATTR));
        let found = StyledString::new(format!("{cast_to}"), Some(ERR), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], hint)],
                switch_lang!(
                    "japanese" => format!("{name}の型を{found}にキャストすることはできません"),
                    "simplified_chinese" => format!("{name}的类型无法转换为{found}"),
                    "traditional_chinese" => format!("{name}的類型無法轉換為{found}"),
                    "english" => format!("the type of {name} cannot be cast to {found}"),
                ),
                errno,
                TypeError,
                loc,
            ),
            input,
            caused_by,
        )
    }
}

impl LowerWarning {
    pub fn unused_warning(
        input: Input,
        errno: usize,
        loc: Location,
        name: &str,
        caused_by: String,
    ) -> Self {
        let name = StyledString::new(readable_name(name), Some(WARN), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("{name}は使用されていません"),
                    "simplified_chinese" => format!("{name}未使用"),
                    "traditional_chinese" => format!("{name}未使用"),
                    "english" => format!("{name} is not used"),
                ),
                errno,
                UnusedWarning,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn union_return_type_warning(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        fn_name: &str,
        typ: &Type,
    ) -> Self {
        let hint = switch_lang!(
            "japanese" => format!("`{fn_name}(...): {typ} = ...`など明示的に戻り値型を指定してください"),
            "simplified_chinese" => format!("请明确指定函数{fn_name}的返回类型，例如`{fn_name}(...): {typ} = ...`"),
            "traditional_chinese" => format!("請明確指定函數{fn_name}的返回類型，例如`{fn_name}(...): {typ} = ...`"),
            "english" => format!("please explicitly specify the return type of function {fn_name}, for example `{fn_name}(...): {typ} = ...`"),
        );
        LowerError::new(
            ErrorCore::new(
                vec![SubMessage::ambiguous_new(loc, vec![], Some(hint))],
                switch_lang!(
                    "japanese" => format!("関数{fn_name}の戻り値型が単一ではありません"),
                    "simplified_chinese" => format!("函数{fn_name}的返回类型不是单一的"),
                    "traditional_chinese" => format!("函數{fn_name}的返回類型不是單一的"),
                    "english" => format!("the return type of function {fn_name} is not single"),
                ),
                errno,
                TypeWarning,
                loc,
            ),
            input,
            caused_by,
        )
    }

    pub fn builtin_exists_warning(
        input: Input,
        errno: usize,
        loc: Location,
        caused_by: String,
        name: &str,
    ) -> Self {
        let name = StyledStr::new(readable_name(name), Some(WARN), Some(ATTR));
        Self::new(
            ErrorCore::new(
                vec![SubMessage::only_loc(loc)],
                switch_lang!(
                    "japanese" => format!("同名の組み込み関数{name}が既に存在します"),
                    "simplified_chinese" => format!("已存在同名的内置函数{name}"),
                    "traditional_chinese" => format!("已存在同名的內置函數{name}"),
                    "english" => format!("a built-in function named {name} already exists"),
                ),
                errno,
                NameWarning,
                loc,
            ),
            input,
            caused_by,
        )
    }
}
