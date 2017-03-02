use std::collections::BTreeMap;
use std::env;
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::iter::Iterator;
use std::mem;

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
        let trimmed = s.trim();

        if trimmed.is_empty() {
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

        Some(LexicalPattern::Statement(Statement::Execution(trimmed.split_whitespace().map(String::from).collect())))
    }
}

pub fn name_valid(name: &str) -> bool {
    let ok = name.chars().nth(0).map_or(false, |c| !c.is_digit(10));
    if !ok {
        return false
    }
    let bad = &['{', '}', '(', ')', '='] as &[_];

    name.find(bad).is_none()
}

#[derive(Clone, Debug)]
enum ParseState {
    ConstructFunc(String, Function),
    Outside,
}

impl ParseState {
    pub fn transform(self, pattern: LexicalPattern, program: &mut Program) -> errors::Result<ParseState> {
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

    pub fn transform_in_place(&mut self, pattern: LexicalPattern, program: &mut Program) -> errors::Result<()> {
        *self = mem::replace(self, ParseState::Outside).transform(pattern, program)?;

        Ok(())
    }

    pub fn end_success(self) -> errors::Result<()> {
        match self {
            ParseState::ConstructFunc(..) => bail!(errors::ErrorKind::InvalidProgram("haven't end".to_owned())),
            ParseState::Outside => Ok(()),
        }
    }
}

struct Environment {
    table: BTreeMap<String, String>,
}

impl Environment {
    pub fn new() -> Environment {
        let table = BTreeMap::new();

        Environment { table: table }
    }

    pub fn exec_assignment(&mut self, assignment: &Assignment) {
        if let Some(variable) = self.table.get_mut(&assignment.variable) {
            *variable = assignment.value.clone();
            return
        }
        self.table.insert(assignment.variable.clone(), assignment.value.clone());
    }

    pub fn exec_execution(&self, args: &[String]) {
        let cmdline: Vec<String> = args.into_iter().map(|x| self.expand(x).to_owned()).collect();

        if !cmdline.is_empty() {
            let (exec, argv) = cmdline.split_at(1);
            let _ = ::std::process::Command::new(&exec[0])
                .args(argv)
                .status()
                .map_err(|e| println!("Command failed: {}", e));
        }
    }

    pub fn exec_statement(&mut self, statement: &Statement) {
        match *statement {
            Statement::Assignment(ref assignment) => {
                self.exec_assignment(assignment)
            },
            Statement::Execution(ref args) => {
                self.exec_execution(args)
            },
        }
    }

    pub fn exec_function(&mut self, function: &Function) {
        for statement in &function.0 {
            self.exec_statement(statement)
        }
    }

    fn expand<'a>(&'a self, arg: &'a str) -> &'a str {
        if arg.starts_with('$') {
            self.table.get(&arg[1..]).map(String::as_str).unwrap_or("")
        } else {
            arg
        }
    }
}

pub fn parse_file_to_ast(filename: &str) -> errors::Result<Program> {
    let mut cwd = env::current_dir()?;
    cwd.push(filename);

    let f = File::open(cwd)?;
    let f = BufReader::new(f);

    parse_to_ast(f.lines())
}

fn parse_to_ast<T, I>(content: T) -> errors::Result<Program>
    where T: IntoIterator<Item=::std::io::Result<I>>,
          I: AsRef<str>
{
    let mut program = BTreeMap::new();

    let mut parser = ParseState::Outside;
    for l in content {
        if let Some(p) = LexicalPattern::from_line(l?.as_ref()) {
            parser.transform_in_place(p, &mut program)?;
        } else {
            bail!(errors::ErrorKind::InvalidProgram("encounter bad line".to_owned()));
        }
    }
    parser.end_success()?;

    Ok(program)
}

pub fn run(filename: &str) -> errors::Result<()> {
    let mut env = Environment::new();

    let program = parse_file_to_ast(filename)?;
    if let Some(main_func) = program.get("main") {
        env.exec_function(main_func);
        return Ok(());
    } else {
        bail!(errors::ErrorKind::InvalidProgram("no main".to_owned()));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_name_valid() {
        let tests = [
            ("hello", true),
            ("你好", true),
            ("hello_123", true),
            ("hello{", false),
            ("hello(", false),
            ("hel}lo", false),
            ("hel)lo", false),
            ("1e", false),
            ("1ist", false),
            ("", false),
        ];

        for t in &tests {
            assert_eq!(t.1, name_valid(t.0), "input: {}", t.1);
        }
    }
}
