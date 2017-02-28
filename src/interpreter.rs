use std::collections::BTreeMap;
use std::env;
use std::io::Read;
use std::fs::File;
use std::iter::Iterator;

use super::errors;

#[derive(Clone, Debug)]
pub struct Assignment {
    pub variable: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub enum Statement {
    Assignment(Assignment),
    Execution(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Function(Vec<Statement>);

pub type Program = BTreeMap<String, Function>;

#[derive(Clone, Debug)]
enum LexicalPattern {
    FuncStart(String),
    FuncEnd,
    Statement(Statement),
    Empty,
}

impl LexicalPattern {
    fn from_line(s: &str) -> Option<LexicalPattern> {
        let trimmed = (&s).trim();

        if trimmed.len() == 0 {
            return Some(LexicalPattern::Empty);
        }

        if trimmed == "}" {
            return Some(LexicalPattern::FuncEnd);
        }

        if trimmed.ends_with("(){") {
            let func_name = &trimmed[..trimmed.len() - 3];
            if !name_valid(func_name) {
                return None;
            } else {
                return Some(LexicalPattern::FuncStart(func_name.to_owned()));
            }
        }

        if let Some(idx) = trimmed.find('=') {
            let var_name = &trimmed[..idx];

            if !name_valid(var_name) {
                return None;
            } else {
                return Some(LexicalPattern::Statement(Statement::Assignment(Assignment { variable: var_name.to_owned(), value: (&trimmed[idx + 1..]).to_owned() })));
            }
        }

        return Some(LexicalPattern::Statement(Statement::Execution(trimmed.split_whitespace().map(String::from).collect())));
    }
}

fn name_valid(_name: &str) -> bool {
    true
}

#[derive(Clone, Debug)]
enum ParseState {
    ConstructFunc(String, Function),
    Outside,
}

impl ParseState {
    fn transform(self, pattern: LexicalPattern, program: &mut Program) -> errors::Result<ParseState> {
        let new_state = match pattern {
            LexicalPattern::FuncStart(name) => {
                match self {
                    ParseState::ConstructFunc(_, _) => bail!(errors::ErrorKind::InvalidProgram("already started to construct function, but got function start again".to_owned())),
                    ParseState::Outside => ParseState::ConstructFunc(name, Function(vec![])),
                }
            },
            LexicalPattern::FuncEnd => {
                match self {
                    ParseState::ConstructFunc(n, f) => {
                        program.insert(n, f);
                        ParseState::Outside
                    },
                    ParseState::Outside => bail!(errors::ErrorKind::InvalidProgram("expect start mark when outside, but got function end".to_owned())),
                }
            },
            LexicalPattern::Statement(statement) => {
                match self {
                    ParseState::ConstructFunc(n, mut f) => {
                        f.0.push(statement);
                        ParseState::ConstructFunc(n, f)
                    },
                    ParseState::Outside => bail!(errors::ErrorKind::InvalidProgram("expect start mark when outside, but got statement".to_owned())),
                }
            },
            LexicalPattern::Empty => {
                self
            },
        };

        Ok(new_state)
    }

    fn transform_in_place(&mut self, pattern: LexicalPattern, program: &mut Program) -> errors::Result<()> {
        let new_state = self.clone();
        *self = new_state.transform(pattern, program)?;

        Ok(())
    }

    fn end_success(self) -> errors::Result<()> {
        match self {
            ParseState::ConstructFunc(_, _) => bail!(errors::ErrorKind::InvalidProgram("haven't end".to_owned())),
            ParseState::Outside => Ok(()),
        }
    }
}

pub fn init_env() -> (BTreeMap<String, String>, ) {
    let table = BTreeMap::new();

    (table, )
}

pub fn parse_file_to_ast(filename: &str) -> errors::Result<Program> {
    let mut cwd = env::current_dir()?;
    cwd.push(filename);

    let mut script_file = File::open(cwd)?;
    let mut contents = String::new();
    script_file.read_to_string(&mut contents)?;

    parse_to_ast(&contents)
}

fn parse_to_ast(content: &str) -> errors::Result<Program> {
    let mut program = BTreeMap::new();

    let mut parser = ParseState::Outside;
    for l in content.lines() {
        if let Some(p) = LexicalPattern::from_line(l) {
            parser.transform_in_place(p, &mut program)?;
        }
    }
    parser.end_success()?;

    Ok(program)
}

pub fn run(filename: &str) -> errors::Result<()> {
    let (mut table, ) = init_env();

    let program = parse_file_to_ast(filename)?;

    if let Some(main_func) = program.get("main") {
        for statement in &main_func.0 {
            match *statement {
                Statement::Assignment(ref assignment) => {
                    if let Some(variable) = table.get_mut(&assignment.variable) {
                        *variable = assignment.value.clone();
                        continue
                    }
                    table.insert(assignment.variable.clone(), assignment.value.clone());
                },
                Statement::Execution(ref args) => {
                    let cmdline: Vec<String> = args.into_iter().map(|x| translate(x, &table)).collect();

                    if cmdline.len() > 1 {
                        let (exec, argv) = cmdline.split_at(1);
                        let _ = ::std::process::Command::new(&exec[0])
                            .args(argv)
                            .status()
                            .map_err(|e| println!("Command failed: {}", e));
                    }
                },
            }
        }
        return Ok(());
    } else {
        bail!(errors::ErrorKind::InvalidProgram("no main".to_owned()));
    }
}

fn translate(arg: &str, table: &BTreeMap<String, String>) -> String {
    if arg.starts_with("$") {
        return table.get(&arg[1..]).map(String::as_str).unwrap_or("").to_owned()
    }

    arg.to_owned()
}