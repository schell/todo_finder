use std::collections::{HashMap, HashSet};

use super::source::TodoParserConfig;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommentStyle {
    Single(String),
    Multi(String, String),
    Border(String),
}

fn from_single(s: &str) -> CommentStyle {
    CommentStyle::Single(s.into())
}

fn from_multi(prefix: &str, suffix: &str) -> CommentStyle {
    CommentStyle::Multi(prefix.into(), suffix.into())
}

fn from_border(border: &str) -> CommentStyle {
    CommentStyle::Border(border.into())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SupportedLanguage {
    pub name: String,
    pub comment_styles: Vec<CommentStyle>,
    pub file_extensions: Vec<String>,
}

impl SupportedLanguage {
    pub fn as_todo_parser_config(&self) -> TodoParserConfig {
        TodoParserConfig::from_comment_styles(self.comment_styles.clone())
    }
}

pub fn lang(name: &str, comment_styles: Vec<CommentStyle>, exts: Vec<&str>) -> SupportedLanguage {
    SupportedLanguage {
        name: name.into(),
        comment_styles,
        file_extensions: exts.into_iter().map(|ext| ext.into()).collect(),
    }
}

pub fn haskell_style() -> Vec<CommentStyle> {
    vec![from_single("--"), from_multi("{-", "-}"), from_border("|")]
}

pub fn nix_style() -> Vec<CommentStyle> {
    vec![from_single("#")]
}

pub fn yml_style() -> Vec<CommentStyle> {
    vec![from_single("#")]
}

pub fn c_style() -> Vec<CommentStyle> {
    vec![
        from_single("//"),
        from_single("///"),
        from_multi("/*", "*/"),
        from_border("*"),
    ]
}

pub fn rust_style() -> Vec<CommentStyle> {
    c_style()
}

pub fn objc_style() -> Vec<CommentStyle> {
    let mut c = c_style();
    c.extend(vec![from_border("!")]);
    c
}

pub fn swift_style() -> Vec<CommentStyle> {
    let mut objc = objc_style();
    objc.extend(vec![from_border(":")]);
    objc
}

pub fn lisp_style() -> Vec<CommentStyle> {
    vec![from_single(";"), from_border(";")]
}

pub fn coffee_style() -> Vec<CommentStyle> {
    vec![from_single("#"), from_multi("###", "###")]
}

pub fn delphi_style() -> Vec<CommentStyle> {
    vec![
        from_single("//"),
        from_multi("{", "}"),
        from_multi("{*", "*}"),
    ]
}

pub fn php_style() -> Vec<CommentStyle> {
    vec![from_single("//"), from_single("#"), from_multi("/*", "*/")]
}

pub fn python_style() -> Vec<CommentStyle> {
    vec![from_single("#"), from_multi("\"\"\"", "\"\"\"")]
}

pub fn all_supported_langs() -> HashSet<SupportedLanguage> {
    vec![
        lang("Actionscript", c_style(), vec!["as"]),
        lang("Apex class", c_style(), vec!["cls"]),
        lang("Apex trigger", c_style(), vec!["trigger"]),
        lang(
            "Applescript",
            vec![from_single("--")],
            vec!["scpt", "applescript"],
        ),
        lang("Assembly", vec![from_single(";")], vec!["asm"]),
        lang("Basic", vec![from_single("REM")], vec!["bas"]),
        lang("Boot", vec![from_single(";")], vec!["boot"]),
        lang(
            "C, C++, C#",
            c_style(),
            vec![
                "h", "c", "cpp", "cs", "cxx", "cc", "hpp", "hxx", "hh", "ino",
            ],
        ),
        lang("Clojure", lisp_style(), vec!["clj", "cljs", "cljc", "edn"]),
        lang("Cmake", vec![from_single("#")], vec!["cmake"]),
        lang("CoffeeScript", coffee_style(), vec!["coffee", "litcoffee"]),
        lang("Cs", c_style(), vec!["cs"]),
        lang("CSS", vec![from_multi("/*", "*/")], vec!["css"]),
        lang("D", vec![from_single("//")], vec!["d"]),
        lang(
            "Delphi, Object Pascal",
            delphi_style(),
            vec!["p", "pp", "pas"],
        ),
        lang("Dos", vec![from_single("@?rem")], vec!["bat", "btm", "cmd"]),
        lang("Earl-grey", vec![from_single(";;")], vec!["eg"]),
        lang("Erlang", vec![from_single("%")], vec!["erl", "hrl"]),
        lang(
            "Gams",
            vec![
                from_single("*"),
                from_multi("$ontext", "$offtext"),
                from_border("-"),
            ],
            vec!["gms"],
        ),
        lang("Go", c_style(), vec!["go"]),
        lang("Groovy", c_style(), vec!["groovy"]),
        lang("Haml", vec![from_single("-#")], vec!["haml"]),
        lang(
            "Haskell, Idris, Purescript, Elm",
            haskell_style(),
            vec!["hs", "purs", "elm", "idr"],
        ),
        lang("Haxe", c_style(), vec!["hx"]),
        lang("HTML", vec![from_multi("<!--", "-->")], vec!["html"]),
        lang("Ini", vec![from_single(";")], vec!["ini"]),
        lang("Jade", vec![from_single("//-")], vec!["jade"]),
        lang("Jade", vec![from_single("//-")], vec!["pug"]),
        lang("Java", c_style(), vec!["java"]),
        lang("JavaScript", c_style(), vec!["js", "es6", "es", "jsx"]),
        lang(
            "Julia",
            vec![from_single("#"), from_multi("#=", "=#"), from_border("#")],
            vec!["jl"],
        ),
        lang("Less", c_style(), vec!["less"]),
        lang("LISP", lisp_style(), vec!["lisp"]),
        lang(
            "Lua",
            vec![from_single("--"), from_multi("--[[", "]]")],
            vec!["lua"],
        ),
        lang("M4", vec![from_single("#")], vec!["m4"]),
        lang(
            "Matlab",
            vec![from_single("%"), from_multi("%{", "%}")],
            vec!["m"],
        ),
        lang("Mel", vec![from_single("//")], vec!["mel"]),
        lang("Nix", nix_style(), vec!["nix"]),
        lang("Objective-C", objc_style(), vec!["h", "m", "mm"]),
        lang(
            "Perl",
            vec![from_single("#")],
            vec!["pl", "pm", "t", "pod", "pl6", "pm6"],
        ),
        lang(
            "PHP",
            php_style(),
            vec![
                "php", "phtml", "php3", "php4", "php5", "php7", "phps", "php-s",
            ],
        ),
        lang(
            "Pkb",
            vec![from_single("--"), from_multi("/*", "*/"), from_border("*")],
            vec!["pkb"],
        ),
        lang(
            "Pks",
            vec![from_single("--"), from_multi("/*", "*/"), from_border("*")],
            vec!["pks"],
        ),
        lang(
            "Powershell",
            vec![from_single("#"), from_multi("<#", "#>"), from_border("#")],
            vec!["ps1"],
        ),
        lang("Properties", vec![from_single("#")], vec!["properties"]),
        lang("Python", python_style(), vec!["py"]),
        lang(
            "R",
            vec![from_single("#")],
            vec!["r", "rdata", "rds", "rda"],
        ),
        lang("Reasonml", c_style(), vec!["re"]),
        lang(
            "Ruby",
            vec![from_single("#"), from_multi("=begin", "=end")],
            vec!["rb"],
        ),
        lang("Rust", c_style(), vec!["rs", "rc"]),
        lang("Sbt", c_style(), vec!["sbt"]),
        lang("Scala", c_style(), vec!["sc", "scala"]),
        lang("Scss", vec![from_single("//")], vec!["scss"]),
        lang("Shell", vec![from_single("#")], vec!["sh", "bash"]),
        lang("Sql", vec![from_single("--")], vec!["sql"]),
        lang("Stylus", vec![from_single("//")], vec!["styl"]),
        lang("Swift", swift_style(), vec!["swift"]),
        lang("Terraform", vec![from_single("#")], vec!["tf"]),
        lang("TeX", vec![from_single("%")], vec!["tex", "latex"]),
        lang("Typescript", c_style(), vec!["ts"]),
        lang("Vala", vec![from_single("//")], vec!["vala", "vapi"]),
        lang(
            "Vbscript",
            vec![from_single("'")],
            vec!["vbe", "vbs", "wsc", "wsf"],
        ),
        lang(
            "Velocity",
            vec![from_single("##"), from_multi("#**", "*#"), from_border("*")],
            vec!["vm"],
        ),
        lang("Vhdl", vec![from_single("--")], vec!["vhdl"]),
        lang("Vim script", vec![from_single("\"")], vec!["vimrc", "vim"]),
        lang("Vue component", c_style(), vec!["vue"]),
        lang("YAML", yml_style(), vec!["yaml", "yml"]),
        lang("Yarn lock", vec![from_single("#")], vec!["lock"]),
    ]
    .into_iter()
    .collect()
}

pub fn language_map() -> HashMap<String, Vec<SupportedLanguage>> {
    let mut lang_map = HashMap::new();
    for language in all_supported_langs().into_iter() {
        for ext in language.file_extensions.iter() {
            let langs_by_ext = lang_map.entry(ext.clone()).or_insert(vec![]);
            langs_by_ext.push(language.clone());
        }
    }
    lang_map
}
