use std::cell::RefCell;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::rc::Rc;

use crate::ast::{Block, Expression, Program, Statement};
use crate::lexer::token::TokenKind;
use crate::value::{Builtin, Environment, FunctionValue, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub message: String,
}

impl RuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}

/// 语句执行时除了普通完成，还可能把控制流传递给外层。
enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

enum LValue {
    Variable {
        environment: Environment,
        name: String,
    },
    ArrayIndex {
        array: Rc<RefCell<Vec<Value>>>,
        index: usize,
    },
    ObjectKey {
        object: Rc<RefCell<std::collections::HashMap<String, Value>>>,
        key: String,
    },
}

pub struct Runtime {
    program: Program,
    globals: Environment,
    output: Vec<String>,
    echo_output: bool,
}

impl Runtime {
    pub fn new(program: Program) -> Self {
        Self::with_output(program, true)
    }

    fn with_output(program: Program, echo_output: bool) -> Self {
        let globals = Environment::global();
        // 内置函数是常量，用户不能用赋值覆盖，但可以在子作用域遮蔽。
        globals
            .define("print".into(), Value::Builtin(Builtin::Print), false)
            .expect("fresh global environment");
        globals
            .define("len".into(), Value::Builtin(Builtin::Len), false)
            .expect("fresh global environment");
        globals
            .define("range".into(), Value::Builtin(Builtin::Range), false)
            .expect("fresh global environment");
        // read/write 保留为简短别名，推荐使用含义更明确的 *_file 名称。
        for (name, builtin) in [
            ("read_file", Builtin::ReadFile),
            ("read", Builtin::ReadFile),
            ("write_file", Builtin::WriteFile),
            ("write", Builtin::WriteFile),
            ("append_file", Builtin::AppendFile),
            ("file_exists", Builtin::FileExists),
            ("is_file", Builtin::IsFile),
            ("is_dir", Builtin::IsDir),
            ("list_dir", Builtin::ListDir),
        ] {
            globals
                .define(name.into(), Value::Builtin(builtin), false)
                .expect("fresh global environment");
        }
        for (name, builtin) in [
            ("str", Builtin::Str),
            ("upper", Builtin::Upper),
            ("lower", Builtin::Lower),
            ("trim", Builtin::Trim),
            ("contains", Builtin::Contains),
            ("starts_with", Builtin::StartsWith),
            ("ends_with", Builtin::EndsWith),
            ("split", Builtin::Split),
            ("join", Builtin::Join),
            ("replace", Builtin::Replace),
            ("abs", Builtin::Abs),
            ("min", Builtin::Min),
            ("max", Builtin::Max),
            ("pow", Builtin::Pow),
            ("sqrt", Builtin::Sqrt),
            ("floor", Builtin::Floor),
            ("ceil", Builtin::Ceil),
            ("round", Builtin::Round),
        ] {
            globals
                .define(name.into(), Value::Builtin(builtin), false)
                .expect("fresh global environment");
        }
        Self {
            program,
            globals,
            output: Vec::new(),
            echo_output,
        }
    }

    /// 执行完整程序。顶层出现 return/break/continue 会转成运行时错误。
    pub fn run(mut self) -> Result<(), RuntimeError> {
        let statements = std::mem::take(&mut self.program.statements);
        match self.execute_statements(&statements, self.globals.clone())? {
            Flow::Normal => Ok(()),
            Flow::Return(_) => Err(RuntimeError::new("`return` used outside a function")),
            Flow::Break => Err(RuntimeError::new("`break` used outside a loop")),
            Flow::Continue => Err(RuntimeError::new("`continue` used outside a loop")),
        }
    }

    fn execute_statements(
        &mut self,
        statements: &[Statement],
        environment: Environment,
    ) -> Result<Flow, RuntimeError> {
        for statement in statements {
            let flow = self.execute_statement(statement, environment.clone())?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    fn execute_block(
        &mut self,
        statements: &Block,
        parent: Environment,
    ) -> Result<Flow, RuntimeError> {
        self.execute_statements(statements, parent.child())
    }

    fn execute_statement(
        &mut self,
        statement: &Statement,
        environment: Environment,
    ) -> Result<Flow, RuntimeError> {
        match statement {
            Statement::Variable {
                mutable,
                name,
                value,
            } => {
                let value = self.evaluate(value, environment.clone())?;
                environment
                    .define(name.clone(), value, *mutable)
                    .map_err(RuntimeError::new)?;
                Ok(Flow::Normal)
            }
            Statement::Function {
                name,
                parameters,
                body,
            } => {
                // closure 指向当前环境；函数放入该环境后也能通过名称递归调用自己。
                let function = FunctionValue {
                    name: name.clone(),
                    parameters: parameters.clone(),
                    body: body.clone(),
                    closure: environment.clone(),
                };
                environment
                    .define(name.clone(), Value::Function(Rc::new(function)), false)
                    .map_err(RuntimeError::new)?;
                Ok(Flow::Normal)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.evaluate(condition, environment.clone())?.is_truthy() {
                    self.execute_block(then_branch, environment)
                } else if let Some(else_branch) = else_branch {
                    self.execute_block(else_branch, environment)
                } else {
                    Ok(Flow::Normal)
                }
            }
            Statement::While { condition, body } => {
                while self.evaluate(condition, environment.clone())?.is_truthy() {
                    match self.execute_block(body, environment.clone())? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::For {
                name,
                iterable,
                body,
            } => {
                let iterable = self.evaluate(iterable, environment.clone())?;
                let values = self.iter_values(iterable)?;
                for value in values {
                    // 每轮拥有独立作用域，闭包捕获循环变量时不会共享最后一个值。
                    let iteration = environment.child();
                    iteration
                        .define(name.clone(), value, true)
                        .map_err(RuntimeError::new)?;
                    match self.execute_statements(body, iteration)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
                Ok(Flow::Normal)
            }
            Statement::Return(value) => {
                let value = match value {
                    Some(value) => self.evaluate(value, environment)?,
                    None => Value::Null,
                };
                Ok(Flow::Return(value))
            }
            Statement::Break => Ok(Flow::Break),
            Statement::Continue => Ok(Flow::Continue),
            Statement::Expression(expression) => {
                self.evaluate(expression, environment)?;
                Ok(Flow::Normal)
            }
            Statement::Block(block) => self.execute_block(block, environment),
        }
    }

    fn evaluate(
        &mut self,
        expression: &Expression,
        environment: Environment,
    ) -> Result<Value, RuntimeError> {
        match expression {
            Expression::Integer(value) => Ok(Value::Int(*value)),
            Expression::Float(value) => Ok(Value::Float(*value)),
            Expression::String(value) => Ok(Value::String(value.clone())),
            Expression::Boolean(value) => Ok(Value::Bool(*value)),
            Expression::Null => Ok(Value::Null),
            Expression::Identifier(name) => environment
                .get(name)
                .ok_or_else(|| RuntimeError::new(format!("undefined name `{name}`"))),
            Expression::Array(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    values.push(self.evaluate(element, environment.clone())?);
                }
                Ok(Value::Array(Rc::new(RefCell::new(values))))
            }
            Expression::Object(entries) => {
                let mut values = std::collections::HashMap::with_capacity(entries.len());
                for (key, expression) in entries {
                    let value = self.evaluate(expression, environment.clone())?;
                    values.insert(key.clone(), value);
                }
                Ok(Value::Object(Rc::new(RefCell::new(values))))
            }
            Expression::Unary { operator, operand } => {
                let operand = self.evaluate(operand, environment)?;
                self.evaluate_unary(operator, operand)
            }
            Expression::Binary {
                left,
                operator,
                right,
            } => self.evaluate_binary(left, operator, right, environment),
            Expression::Assignment {
                target,
                operator,
                value,
            } => {
                // 先解析左值，确保索引表达式只执行一次。
                let target = self.resolve_lvalue(target, environment.clone())?;
                let right = self.evaluate(value, environment)?;
                let result = if *operator == TokenKind::Equal {
                    right
                } else {
                    let left = self.read_lvalue(&target)?;
                    let binary_operator = compound_operator(operator).expect("assignment operator");
                    self.apply_binary(binary_operator, left, right)?
                };
                self.write_lvalue(target, result.clone())?;
                Ok(result)
            }
            Expression::Call { callee, arguments } => {
                let callee = self.evaluate(callee, environment.clone())?;
                let mut values = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    values.push(self.evaluate(argument, environment.clone())?);
                }
                self.call(callee, values)
            }
            Expression::Index { object, index } => {
                let object = self.evaluate(object, environment.clone())?;
                let index = self.evaluate(index, environment)?;
                self.read_index(object, index)
            }
            Expression::Member { object, property } => {
                let object = self.evaluate(object, environment)?;
                match object {
                    Value::Object(object) => {
                        object.borrow().get(property).cloned().ok_or_else(|| {
                            RuntimeError::new(format!("object has no member `{property}`"))
                        })
                    }
                    value => Err(RuntimeError::new(format!(
                        "type `{}` has no members",
                        value.type_name()
                    ))),
                }
            }
        }
    }

    fn evaluate_unary(&self, operator: &TokenKind, value: Value) -> Result<Value, RuntimeError> {
        match operator {
            TokenKind::Bang => Ok(Value::Bool(!value.is_truthy())),
            TokenKind::Minus => match value {
                Value::Int(value) => value
                    .checked_neg()
                    .map(Value::Int)
                    .ok_or_else(|| RuntimeError::new("integer overflow in unary `-`")),
                Value::Float(value) => Ok(Value::Float(-value)),
                value => Err(unary_type_error("-", &value)),
            },
            TokenKind::Plus => match value {
                value @ (Value::Int(_) | Value::Float(_)) => Ok(value),
                value => Err(unary_type_error("+", &value)),
            },
            TokenKind::Tilde => match value {
                Value::Int(value) => Ok(Value::Int(!value)),
                value => Err(unary_type_error("~", &value)),
            },
            _ => unreachable!("parser only creates supported unary operators"),
        }
    }

    fn evaluate_binary(
        &mut self,
        left: &Expression,
        operator: &TokenKind,
        right: &Expression,
        environment: Environment,
    ) -> Result<Value, RuntimeError> {
        let left = self.evaluate(left, environment.clone())?;
        // 逻辑运算短路，并像 Python 的 and/or 一样返回其中一个操作数。
        match operator {
            TokenKind::AndAnd if !left.is_truthy() => return Ok(left),
            TokenKind::OrOr if left.is_truthy() => return Ok(left),
            TokenKind::AndAnd | TokenKind::OrOr => {
                return self.evaluate(right, environment);
            }
            _ => {}
        }
        let right = self.evaluate(right, environment)?;
        self.apply_binary(operator, left, right)
    }

    fn apply_binary(
        &self,
        operator: &TokenKind,
        left: Value,
        right: Value,
    ) -> Result<Value, RuntimeError> {
        match operator {
            TokenKind::Plus => add_values(left, right),
            TokenKind::Minus => numeric_values("-", left, right, i64::checked_sub, |a, b| a - b),
            TokenKind::Star => multiply_values(left, right),
            TokenKind::Slash => divide_values(left, right),
            TokenKind::Percent => modulo_values(left, right),
            TokenKind::EqualEqual => Ok(Value::Bool(values_equal(&left, &right))),
            TokenKind::BangEqual => Ok(Value::Bool(!values_equal(&left, &right))),
            TokenKind::Less => compare_values("<", left, right, |order| order.is_lt()),
            TokenKind::LessEqual => compare_values("<=", left, right, |order| order.is_le()),
            TokenKind::Greater => compare_values(">", left, right, |order| order.is_gt()),
            TokenKind::GreaterEqual => compare_values(">=", left, right, |order| order.is_ge()),
            TokenKind::Ampersand => integer_binary("&", left, right, |a, b| a & b),
            TokenKind::Pipe => integer_binary("|", left, right, |a, b| a | b),
            TokenKind::Caret => integer_binary("^", left, right, |a, b| a ^ b),
            TokenKind::ShiftLeft => shift_values("<<", left, right, true),
            TokenKind::ShiftRight => shift_values(">>", left, right, false),
            _ => unreachable!("logical and assignment operators are handled separately"),
        }
    }

    fn call(&mut self, callee: Value, arguments: Vec<Value>) -> Result<Value, RuntimeError> {
        match callee {
            Value::Function(function) => {
                if arguments.len() != function.parameters.len() {
                    return Err(RuntimeError::new(format!(
                        "function `{}` expected {} arguments, got {}",
                        function.name,
                        function.parameters.len(),
                        arguments.len()
                    )));
                }
                let local = function.closure.child();
                for (name, value) in function.parameters.iter().zip(arguments) {
                    local
                        .define(name.clone(), value, true)
                        .map_err(RuntimeError::new)?;
                }
                match self.execute_statements(&function.body, local)? {
                    Flow::Normal => Ok(Value::Null),
                    Flow::Return(value) => Ok(value),
                    Flow::Break => Err(RuntimeError::new("`break` cannot leave a function")),
                    Flow::Continue => Err(RuntimeError::new("`continue` cannot leave a function")),
                }
            }
            Value::Builtin(builtin) => self.call_builtin(builtin, arguments),
            value => Err(RuntimeError::new(format!(
                "type `{}` is not callable",
                value.type_name()
            ))),
        }
    }

    fn call_builtin(
        &mut self,
        builtin: Builtin,
        arguments: Vec<Value>,
    ) -> Result<Value, RuntimeError> {
        match builtin {
            Builtin::Print => {
                let line = arguments
                    .iter()
                    .map(Value::display)
                    .collect::<Vec<_>>()
                    .join(" ");
                if self.echo_output {
                    println!("{line}");
                }
                self.output.push(line);
                Ok(Value::Null)
            }
            Builtin::Len => {
                expect_arity("len", &arguments, 1)?;
                let length = match &arguments[0] {
                    Value::String(value) => value.chars().count(),
                    Value::Array(value) => value.borrow().len(),
                    Value::Object(value) => value.borrow().len(),
                    value => {
                        return Err(RuntimeError::new(format!(
                            "len() does not support `{}`",
                            value.type_name()
                        )));
                    }
                };
                let length = i64::try_from(length)
                    .map_err(|_| RuntimeError::new("length does not fit in int"))?;
                Ok(Value::Int(length))
            }
            Builtin::Range => range_values(arguments),
            Builtin::ReadFile => {
                expect_arity("read_file", &arguments, 1)?;
                let path = expect_string("read_file", &arguments[0])?;
                fs::read_to_string(path)
                    .map(Value::String)
                    .map_err(|error| file_error("read_file", path, error))
            }
            Builtin::WriteFile => {
                expect_arity("write_file", &arguments, 2)?;
                let path = expect_string("write_file", &arguments[0])?;
                let content = expect_string("write_file", &arguments[1])?;
                fs::write(path, content)
                    .map(|()| Value::Null)
                    .map_err(|error| file_error("write_file", path, error))
            }
            Builtin::AppendFile => {
                expect_arity("append_file", &arguments, 2)?;
                let path = expect_string("append_file", &arguments[0])?;
                let content = expect_string("append_file", &arguments[1])?;
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|error| file_error("append_file", path, error))?;
                file.write_all(content.as_bytes())
                    .map_err(|error| file_error("append_file", path, error))?;
                Ok(Value::Null)
            }
            Builtin::FileExists => {
                expect_arity("file_exists", &arguments, 1)?;
                let path = expect_string("file_exists", &arguments[0])?;
                Ok(Value::Bool(std::path::Path::new(path).exists()))
            }
            Builtin::IsFile => {
                expect_arity("is_file", &arguments, 1)?;
                let path = expect_string("is_file", &arguments[0])?;
                Ok(Value::Bool(std::path::Path::new(path).is_file()))
            }
            Builtin::IsDir => {
                expect_arity("is_dir", &arguments, 1)?;
                let path = expect_string("is_dir", &arguments[0])?;
                Ok(Value::Bool(std::path::Path::new(path).is_dir()))
            }
            Builtin::ListDir => {
                expect_arity("list_dir", &arguments, 1)?;
                let path = expect_string("list_dir", &arguments[0])?;
                let entries =
                    fs::read_dir(path).map_err(|error| file_error("list_dir", path, error))?;
                let mut names = Vec::new();
                for entry in entries {
                    let entry = entry.map_err(|error| file_error("list_dir", path, error))?;
                    names.push(Value::String(
                        entry.file_name().to_string_lossy().into_owned(),
                    ));
                }
                names.sort_by_key(Value::display);
                Ok(Value::Array(Rc::new(RefCell::new(names))))
            }
            Builtin::Str => {
                expect_arity("str", &arguments, 1)?;
                Ok(Value::String(arguments[0].display()))
            }
            Builtin::Upper => string_map("upper", &arguments, str::to_uppercase),
            Builtin::Lower => string_map("lower", &arguments, str::to_lowercase),
            Builtin::Trim => string_map("trim", &arguments, |value| value.trim().to_owned()),
            Builtin::Contains => {
                let (value, needle) = expect_two_strings("contains", &arguments)?;
                Ok(Value::Bool(value.contains(needle)))
            }
            Builtin::StartsWith => {
                let (value, prefix) = expect_two_strings("starts_with", &arguments)?;
                Ok(Value::Bool(value.starts_with(prefix)))
            }
            Builtin::EndsWith => {
                let (value, suffix) = expect_two_strings("ends_with", &arguments)?;
                Ok(Value::Bool(value.ends_with(suffix)))
            }
            Builtin::Split => {
                let (value, separator) = expect_two_strings("split", &arguments)?;
                if separator.is_empty() {
                    return Err(RuntimeError::new("split() separator cannot be empty"));
                }
                let parts = value
                    .split(separator)
                    .map(|part| Value::String(part.to_owned()))
                    .collect();
                Ok(Value::Array(Rc::new(RefCell::new(parts))))
            }
            Builtin::Join => join_strings(arguments),
            Builtin::Replace => {
                expect_arity("replace", &arguments, 3)?;
                let value = expect_string("replace", &arguments[0])?;
                let from = expect_string("replace", &arguments[1])?;
                let to = expect_string("replace", &arguments[2])?;
                Ok(Value::String(value.replace(from, to)))
            }
            Builtin::Abs => absolute_value(arguments),
            Builtin::Min => extreme_value("min", arguments, false),
            Builtin::Max => extreme_value("max", arguments, true),
            Builtin::Pow => power_value(arguments),
            Builtin::Sqrt => unary_float_math("sqrt", arguments, |value| {
                if value < 0.0 {
                    Err(RuntimeError::new("sqrt() does not support negative values"))
                } else {
                    Ok(value.sqrt())
                }
            }),
            Builtin::Floor => rounded_integer("floor", arguments, f64::floor),
            Builtin::Ceil => rounded_integer("ceil", arguments, f64::ceil),
            Builtin::Round => rounded_integer("round", arguments, f64::round_ties_even),
        }
    }

    fn iter_values(&self, value: Value) -> Result<Vec<Value>, RuntimeError> {
        match value {
            Value::Array(values) => Ok(values.borrow().clone()),
            Value::String(value) => Ok(value
                .chars()
                .map(|character| Value::String(character.to_string()))
                .collect()),
            Value::Object(values) => {
                let mut keys = values.borrow().keys().cloned().collect::<Vec<_>>();
                keys.sort();
                Ok(keys.into_iter().map(Value::String).collect())
            }
            value => Err(RuntimeError::new(format!(
                "type `{}` is not iterable",
                value.type_name()
            ))),
        }
    }

    fn resolve_lvalue(
        &mut self,
        expression: &Expression,
        environment: Environment,
    ) -> Result<LValue, RuntimeError> {
        match expression {
            Expression::Identifier(name) => {
                let environment = environment
                    .resolve(name)
                    .ok_or_else(|| RuntimeError::new(format!("undefined name `{name}`")))?;
                Ok(LValue::Variable {
                    environment,
                    name: name.clone(),
                })
            }
            Expression::Index { object, index } => {
                let object = self.evaluate(object, environment.clone())?;
                let index = self.evaluate(index, environment)?;
                match (object, index) {
                    (Value::Array(array), Value::Int(index)) => {
                        let index = normalize_index(index, array.borrow().len())?;
                        Ok(LValue::ArrayIndex { array, index })
                    }
                    (Value::Object(object), Value::String(key)) => {
                        Ok(LValue::ObjectKey { object, key })
                    }
                    (object, index) => Err(RuntimeError::new(format!(
                        "cannot assign index `{}` on `{}`",
                        index.type_name(),
                        object.type_name()
                    ))),
                }
            }
            Expression::Member { object, property } => match self.evaluate(object, environment)? {
                Value::Object(object) => Ok(LValue::ObjectKey {
                    object,
                    key: property.clone(),
                }),
                value => Err(RuntimeError::new(format!(
                    "cannot assign member on `{}`",
                    value.type_name()
                ))),
            },
            _ => Err(RuntimeError::new("invalid assignment target")),
        }
    }

    fn read_lvalue(&self, target: &LValue) -> Result<Value, RuntimeError> {
        match target {
            LValue::Variable { environment, name } => environment
                .get_local(name)
                .ok_or_else(|| RuntimeError::new(format!("undefined name `{name}`"))),
            LValue::ArrayIndex { array, index } => Ok(array.borrow()[*index].clone()),
            LValue::ObjectKey { object, key } => object
                .borrow()
                .get(key)
                .cloned()
                .ok_or_else(|| RuntimeError::new(format!("object has no member `{key}`"))),
        }
    }

    fn write_lvalue(&self, target: LValue, value: Value) -> Result<(), RuntimeError> {
        match target {
            LValue::Variable { environment, name } => environment
                .assign_local(&name, value)
                .map_err(RuntimeError::new),
            LValue::ArrayIndex { array, index } => {
                array.borrow_mut()[index] = value;
                Ok(())
            }
            LValue::ObjectKey { object, key } => {
                object.borrow_mut().insert(key, value);
                Ok(())
            }
        }
    }

    fn read_index(&self, object: Value, index: Value) -> Result<Value, RuntimeError> {
        match (object, index) {
            (Value::Array(array), Value::Int(index)) => {
                let index = normalize_index(index, array.borrow().len())?;
                let value = array.borrow()[index].clone();
                Ok(value)
            }
            (Value::String(string), Value::Int(index)) => {
                let characters = string.chars().collect::<Vec<_>>();
                let index = normalize_index(index, characters.len())?;
                Ok(Value::String(characters[index].to_string()))
            }
            (Value::Object(object), Value::String(key)) => object
                .borrow()
                .get(&key)
                .cloned()
                .ok_or_else(|| RuntimeError::new(format!("object has no key `{key}`"))),
            (object, index) => Err(RuntimeError::new(format!(
                "cannot index `{}` with `{}`",
                object.type_name(),
                index.type_name()
            ))),
        }
    }
}

fn compound_operator(operator: &TokenKind) -> Option<&'static TokenKind> {
    match operator {
        TokenKind::PlusEqual => Some(&TokenKind::Plus),
        TokenKind::MinusEqual => Some(&TokenKind::Minus),
        TokenKind::StarEqual => Some(&TokenKind::Star),
        TokenKind::SlashEqual => Some(&TokenKind::Slash),
        TokenKind::PercentEqual => Some(&TokenKind::Percent),
        _ => None,
    }
}

fn add_values(left: Value, right: Value) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::String(left), Value::String(right)) => Ok(Value::String(left + &right)),
        (Value::Array(left), Value::Array(right)) => {
            let mut values = left.borrow().clone();
            values.extend(right.borrow().iter().cloned());
            Ok(Value::Array(Rc::new(RefCell::new(values))))
        }
        (left, right) => numeric_values("+", left, right, i64::checked_add, |a, b| a + b),
    }
}

fn multiply_values(left: Value, right: Value) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::String(value), Value::Int(times)) | (Value::Int(times), Value::String(value)) => {
            repeat_string(value, times)
        }
        (Value::Array(value), Value::Int(times)) | (Value::Int(times), Value::Array(value)) => {
            repeat_array(value, times)
        }
        (left, right) => numeric_values("*", left, right, i64::checked_mul, |a, b| a * b),
    }
}

fn numeric_values(
    operator: &str,
    left: Value,
    right: Value,
    integer: fn(i64, i64) -> Option<i64>,
    float: fn(f64, f64) -> f64,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => integer(left, right)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new(format!("integer overflow in `{operator}`"))),
        (Value::Int(left), Value::Float(right)) => Ok(Value::Float(float(left as f64, right))),
        (Value::Float(left), Value::Int(right)) => Ok(Value::Float(float(left, right as f64))),
        (Value::Float(left), Value::Float(right)) => Ok(Value::Float(float(left, right))),
        (left, right) => Err(binary_type_error(operator, &left, &right)),
    }
}

fn divide_values(left: Value, right: Value) -> Result<Value, RuntimeError> {
    let (left, right) = numeric_pair("/", left, right)?;
    if right == 0.0 {
        return Err(RuntimeError::new("division by zero"));
    }
    Ok(Value::Float(left / right))
}

fn modulo_values(left: Value, right: Value) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Int(_), Value::Int(0)) => Err(RuntimeError::new("modulo by zero")),
        (Value::Int(left), Value::Int(right)) => {
            // Python 使用向负无穷取整的商，因此余数符号跟除数一致。
            // 使用 i128 中间值也能安全处理 i64::MIN % -1。
            let left = i128::from(left);
            let right = i128::from(right);
            let mut quotient = left / right;
            let truncated_remainder = left % right;
            if truncated_remainder != 0 && (left < 0) != (right < 0) {
                quotient -= 1;
            }
            Ok(Value::Int((left - quotient * right) as i64))
        }
        (left, right) => {
            let (left, right) = numeric_pair("%", left, right)?;
            if right == 0.0 {
                return Err(RuntimeError::new("modulo by zero"));
            }
            Ok(Value::Float(left - (left / right).floor() * right))
        }
    }
}

fn numeric_pair(operator: &str, left: Value, right: Value) -> Result<(f64, f64), RuntimeError> {
    let left_type = left.type_name();
    let right_type = right.type_name();
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Ok((left as f64, right as f64)),
        (Value::Int(left), Value::Float(right)) => Ok((left as f64, right)),
        (Value::Float(left), Value::Int(right)) => Ok((left, right as f64)),
        (Value::Float(left), Value::Float(right)) => Ok((left, right)),
        _ => Err(RuntimeError::new(format!(
            "operator `{operator}` does not support `{left_type}` and `{right_type}`"
        ))),
    }
}

fn integer_binary(
    operator: &str,
    left: Value,
    right: Value,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Ok(Value::Int(operation(left, right))),
        (left, right) => Err(binary_type_error(operator, &left, &right)),
    }
}

fn shift_values(
    operator: &str,
    left: Value,
    right: Value,
    left_shift: bool,
) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => {
            let amount = u32::try_from(right)
                .ok()
                .filter(|amount| *amount < 64)
                .ok_or_else(|| RuntimeError::new("shift count must be between 0 and 63"))?;
            let value = if left_shift {
                left.checked_shl(amount)
            } else {
                left.checked_shr(amount)
            }
            .ok_or_else(|| RuntimeError::new(format!("integer overflow in `{operator}`")))?;
            Ok(Value::Int(value))
        }
        (left, right) => Err(binary_type_error(operator, &left, &right)),
    }
}

fn compare_values(
    operator: &str,
    left: Value,
    right: Value,
    predicate: fn(std::cmp::Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    let ordering = match (&left, &right) {
        (Value::Int(left), Value::Int(right)) => left.partial_cmp(right),
        (Value::Int(left), Value::Float(right)) => (*left as f64).partial_cmp(right),
        (Value::Float(left), Value::Int(right)) => left.partial_cmp(&(*right as f64)),
        (Value::Float(left), Value::Float(right)) => left.partial_cmp(right),
        (Value::String(left), Value::String(right)) => Some(left.cmp(right)),
        _ => return Err(binary_type_error(operator, &left, &right)),
    }
    .ok_or_else(|| RuntimeError::new("cannot compare NaN values"))?;
    Ok(Value::Bool(predicate(ordering)))
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::Int(left), Value::Float(right)) => (*left as f64) == *right,
        (Value::Float(left), Value::Int(right)) => *left == (*right as f64),
        (Value::Float(left), Value::Float(right)) => left == right,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Array(left), Value::Array(right)) => {
            let left = left.borrow();
            let right = right.borrow();
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| values_equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            let left = left.borrow();
            let right = right.borrow();
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right
                        .get(key)
                        .is_some_and(|right| values_equal(value, right))
                })
        }
        (Value::Function(left), Value::Function(right)) => Rc::ptr_eq(left, right),
        (Value::Builtin(left), Value::Builtin(right)) => left == right,
        _ => false,
    }
}

fn repeat_string(value: String, times: i64) -> Result<Value, RuntimeError> {
    if times <= 0 {
        return Ok(Value::String(String::new()));
    }
    let times = usize::try_from(times).map_err(|_| RuntimeError::new("repeat count too large"))?;
    Ok(Value::String(value.repeat(times)))
}

fn repeat_array(value: Rc<RefCell<Vec<Value>>>, times: i64) -> Result<Value, RuntimeError> {
    if times <= 0 {
        return Ok(Value::Array(Rc::new(RefCell::new(Vec::new()))));
    }
    let times = usize::try_from(times).map_err(|_| RuntimeError::new("repeat count too large"))?;
    let source = value.borrow();
    let capacity = source
        .len()
        .checked_mul(times)
        .ok_or_else(|| RuntimeError::new("repeated array is too large"))?;
    let mut result = Vec::with_capacity(capacity);
    for _ in 0..times {
        result.extend(source.iter().cloned());
    }
    Ok(Value::Array(Rc::new(RefCell::new(result))))
}

fn normalize_index(index: i64, length: usize) -> Result<usize, RuntimeError> {
    let length = i64::try_from(length).map_err(|_| RuntimeError::new("container is too large"))?;
    let normalized = if index < 0 { length + index } else { index };
    if normalized < 0 || normalized >= length {
        return Err(RuntimeError::new(format!("index {index} out of range")));
    }
    Ok(normalized as usize)
}

fn expect_arity(name: &str, arguments: &[Value], expected: usize) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(RuntimeError::new(format!(
            "{name}() expected {expected} arguments, got {}",
            arguments.len()
        )))
    }
}

fn expect_string<'a>(name: &str, value: &'a Value) -> Result<&'a str, RuntimeError> {
    match value {
        Value::String(value) => Ok(value),
        value => Err(RuntimeError::new(format!(
            "{name}() expected string, got `{}`",
            value.type_name()
        ))),
    }
}

fn expect_two_strings<'a>(
    name: &str,
    arguments: &'a [Value],
) -> Result<(&'a str, &'a str), RuntimeError> {
    expect_arity(name, arguments, 2)?;
    Ok((
        expect_string(name, &arguments[0])?,
        expect_string(name, &arguments[1])?,
    ))
}

fn string_map(
    name: &str,
    arguments: &[Value],
    operation: impl FnOnce(&str) -> String,
) -> Result<Value, RuntimeError> {
    expect_arity(name, arguments, 1)?;
    Ok(Value::String(operation(expect_string(
        name,
        &arguments[0],
    )?)))
}

fn join_strings(arguments: Vec<Value>) -> Result<Value, RuntimeError> {
    expect_arity("join", &arguments, 2)?;
    let separator = expect_string("join", &arguments[0])?;
    let Value::Array(values) = &arguments[1] else {
        return Err(RuntimeError::new(format!(
            "join() expected array, got `{}`",
            arguments[1].type_name()
        )));
    };
    let values = values.borrow();
    let mut strings = Vec::with_capacity(values.len());
    for value in values.iter() {
        strings.push(expect_string("join", value)?);
    }
    Ok(Value::String(strings.join(separator)))
}

fn absolute_value(arguments: Vec<Value>) -> Result<Value, RuntimeError> {
    expect_arity("abs", &arguments, 1)?;
    match &arguments[0] {
        Value::Int(value) => value
            .checked_abs()
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new("integer overflow in abs()")),
        Value::Float(value) => Ok(Value::Float(value.abs())),
        value => Err(RuntimeError::new(format!(
            "abs() expected int or float, got `{}`",
            value.type_name()
        ))),
    }
}

fn extreme_value(
    name: &str,
    arguments: Vec<Value>,
    select_greater: bool,
) -> Result<Value, RuntimeError> {
    let mut values = arguments.into_iter();
    let mut best = values
        .next()
        .ok_or_else(|| RuntimeError::new(format!("{name}() expected at least 1 argument")))?;
    numeric_order(name, &best, &best)?;
    for candidate in values {
        let ordering = numeric_order(name, &candidate, &best)?;
        let replace = if select_greater {
            ordering.is_gt()
        } else {
            ordering.is_lt()
        };
        if replace {
            best = candidate;
        }
    }
    Ok(best)
}

fn power_value(arguments: Vec<Value>) -> Result<Value, RuntimeError> {
    expect_arity("pow", &arguments, 2)?;
    if let (Value::Int(base), Value::Int(exponent)) = (&arguments[0], &arguments[1])
        && *exponent >= 0
    {
        let exponent = u32::try_from(*exponent)
            .map_err(|_| RuntimeError::new("pow() exponent is too large"))?;
        return base
            .checked_pow(exponent)
            .map(Value::Int)
            .ok_or_else(|| RuntimeError::new("integer overflow in pow()"));
    }
    let base = numeric_scalar("pow", &arguments[0])?;
    let exponent = numeric_scalar("pow", &arguments[1])?;
    let result = base.powf(exponent);
    if result.is_finite() {
        Ok(Value::Float(result))
    } else {
        Err(RuntimeError::new(
            "pow() result is not a finite real number",
        ))
    }
}

fn unary_float_math(
    name: &str,
    arguments: Vec<Value>,
    operation: impl FnOnce(f64) -> Result<f64, RuntimeError>,
) -> Result<Value, RuntimeError> {
    expect_arity(name, &arguments, 1)?;
    let value = numeric_scalar(name, &arguments[0])?;
    operation(value).map(Value::Float)
}

fn rounded_integer(
    name: &str,
    arguments: Vec<Value>,
    operation: fn(f64) -> f64,
) -> Result<Value, RuntimeError> {
    expect_arity(name, &arguments, 1)?;
    match &arguments[0] {
        Value::Int(value) => Ok(Value::Int(*value)),
        Value::Float(value) => float_to_integer(name, operation(*value)).map(Value::Int),
        value => Err(RuntimeError::new(format!(
            "{name}() expected int or float, got `{}`",
            value.type_name()
        ))),
    }
}

fn numeric_scalar(name: &str, value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Int(value) => Ok(*value as f64),
        Value::Float(value) => Ok(*value),
        value => Err(RuntimeError::new(format!(
            "{name}() expected int or float, got `{}`",
            value.type_name()
        ))),
    }
}

fn numeric_order(
    name: &str,
    left: &Value,
    right: &Value,
) -> Result<std::cmp::Ordering, RuntimeError> {
    let ordering = match (left, right) {
        (Value::Int(left), Value::Int(right)) => Some(left.cmp(right)),
        (Value::Int(left), Value::Float(right)) => (*left as f64).partial_cmp(right),
        (Value::Float(left), Value::Int(right)) => left.partial_cmp(&(*right as f64)),
        (Value::Float(left), Value::Float(right)) => left.partial_cmp(right),
        (Value::Int(_) | Value::Float(_), right) => {
            return Err(RuntimeError::new(format!(
                "{name}() expected int or float, got `{}`",
                right.type_name()
            )));
        }
        (left, _) => {
            return Err(RuntimeError::new(format!(
                "{name}() expected int or float, got `{}`",
                left.type_name()
            )));
        }
    };
    ordering.ok_or_else(|| RuntimeError::new(format!("{name}() cannot compare NaN")))
}

fn float_to_integer(name: &str, value: f64) -> Result<i64, RuntimeError> {
    let upper_bound = -(i64::MIN as f64);
    if !value.is_finite() || value < i64::MIN as f64 || value >= upper_bound {
        return Err(RuntimeError::new(format!(
            "{name}() result does not fit in int"
        )));
    }
    Ok(value as i64)
}

fn file_error(operation: &str, path: &str, error: std::io::Error) -> RuntimeError {
    RuntimeError::new(format!("{operation}({path:?}) failed: {error}"))
}

fn range_values(arguments: Vec<Value>) -> Result<Value, RuntimeError> {
    if !(1..=3).contains(&arguments.len()) {
        return Err(RuntimeError::new(format!(
            "range() expected 1 to 3 arguments, got {}",
            arguments.len()
        )));
    }
    let mut integers = Vec::with_capacity(arguments.len());
    for argument in arguments {
        match argument {
            Value::Int(value) => integers.push(value),
            value => {
                return Err(RuntimeError::new(format!(
                    "range() arguments must be int, got `{}`",
                    value.type_name()
                )));
            }
        }
    }
    let (mut current, stop, step) = match integers.as_slice() {
        [stop] => (0, *stop, 1),
        [start, stop] => (*start, *stop, 1),
        [start, stop, step] => (*start, *stop, *step),
        _ => unreachable!(),
    };
    if step == 0 {
        return Err(RuntimeError::new("range() step cannot be zero"));
    }
    let mut result = Vec::new();
    while (step > 0 && current < stop) || (step < 0 && current > stop) {
        if result.len() >= 1_000_000 {
            return Err(RuntimeError::new("range() produced too many values"));
        }
        result.push(Value::Int(current));
        current = current
            .checked_add(step)
            .ok_or_else(|| RuntimeError::new("integer overflow in range()"))?;
    }
    Ok(Value::Array(Rc::new(RefCell::new(result))))
}

fn unary_type_error(operator: &str, value: &Value) -> RuntimeError {
    RuntimeError::new(format!(
        "operator `{operator}` does not support `{}`",
        value.type_name()
    ))
}

fn binary_type_error(operator: &str, left: &Value, right: &Value) -> RuntimeError {
    RuntimeError::new(format!(
        "operator `{operator}` does not support `{}` and `{}`",
        left.type_name(),
        right.type_name()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn execute(source: &str) -> Result<Vec<String>, RuntimeError> {
        let program = Parser::new(Lexer::new(source.into())).parse().unwrap();
        let mut runtime = Runtime::with_output(program, false);
        let statements = std::mem::take(&mut runtime.program.statements);
        match runtime.execute_statements(&statements, runtime.globals.clone())? {
            Flow::Normal => Ok(runtime.output),
            _ => Err(RuntimeError::new("unexpected top-level control flow")),
        }
    }

    #[test]
    fn evaluates_arithmetic_truthiness_and_builtins() {
        let output = execute(
            r#"
                print(1 + 2 * 3, 5 / 2);
                print(len("你好"), len([1, 2, 3]));
                print("ha" * 3);
                if [] { print("bad"); } else { print("empty"); }
            "#,
        )
        .unwrap();
        assert_eq!(output, ["7 2.5", "2 3", "hahaha", "empty"]);
    }

    #[test]
    fn runs_functions_closures_and_recursion() {
        let output = execute(
            r#"
                fn factorial(n) {
                    if n <= 1 { return 1; }
                    return n * factorial(n - 1);
                }
                fn make_adder(x) {
                    fn add(y) { return x + y; }
                    return add;
                }
                let add10 = make_adder(10);
                print(factorial(5), add10(7));
            "#,
        )
        .unwrap();
        assert_eq!(output, ["120 17"]);
    }

    #[test]
    fn runs_loops_range_and_control_flow() {
        let output = execute(
            r#"
                let total = 0;
                for i in range(1, 8) {
                    if i == 3 { continue; }
                    if i == 6 { break; }
                    total += i;
                }
                let n = 3;
                while n > 0 { print(n); n -= 1; }
                print(total);
            "#,
        )
        .unwrap();
        assert_eq!(output, ["3", "2", "1", "12"]);
    }

    #[test]
    fn supports_array_aliases_assignment_and_negative_index() {
        let output = execute(
            r#"
                let values = [1, 2, 3];
                let alias = values;
                alias[-1] = 9;
                values[0] += 4;
                print(values, values[-1]);
            "#,
        )
        .unwrap();
        assert_eq!(output, ["[5, 2, 9] 9"]);
    }

    #[test]
    fn supports_objects_and_python_style_modulo() {
        let output = execute(
            r#"
                let user = {name: "mix", "count": 1};
                user.count += 2;
                user["enabled"] = true;
                print(user.name, user.count, user["enabled"]);
                print(-3 % 2, 3 % -2);
            "#,
        )
        .unwrap();
        assert_eq!(output, ["mix 3 true", "1 -1"]);
    }

    #[test]
    fn supports_common_file_operations() {
        let unique = format!(
            "mix-runtime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(unique);
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("example.txt");
        let directory_literal = format!("{:?}", directory.to_string_lossy());
        let file_literal = format!("{:?}", file.to_string_lossy());
        let source = format!(
            r#"
                write_file({file_literal}, "hello");
                append_file({file_literal}, " world");
                print(file_exists({file_literal}));
                print(is_file({file_literal}), is_dir({file_literal}));
                print(is_file({directory_literal}), is_dir({directory_literal}));
                print(read_file({file_literal}));
                print(len(list_dir({directory_literal})));
            "#
        );

        let result = execute(&source);
        std::fs::remove_file(&file).unwrap();
        std::fs::remove_dir(&directory).unwrap();

        assert_eq!(
            result.unwrap(),
            ["true", "true false", "false true", "hello world", "1"]
        );
    }

    #[test]
    fn supports_string_and_math_builtins() {
        let output = execute(
            r#"
                print(upper("Mix"), lower("RUST"), trim("  text  "));
                print(contains("hello", "ell"), starts_with("hello", "he"), ends_with("hello", "lo"));
                print(split("a,b,c", ","));
                print(join("-", ["a", "b", "c"]));
                print(replace("a-b-a", "a", "x"));
                print(str([1, "x"]));
                print(abs(-5), min(3, 1.5, 2), max(3, 1.5, 2));
                print(pow(2, 10), pow(4, -1), sqrt(9));
                print(floor(2.9), ceil(2.1), round(2.5), round(3.5));
            "#,
        )
        .unwrap();
        assert_eq!(
            output,
            [
                "MIX rust text",
                "true true true",
                "[\"a\", \"b\", \"c\"]",
                "a-b-c",
                "x-b-x",
                "[1, \"x\"]",
                "5 1.5 3",
                "1024 0.25 3.0",
                "2 3 2 4",
            ]
        );
    }

    #[test]
    fn rejects_const_assignment_and_bad_calls() {
        let error = execute("const value = 1; value = 2;").unwrap_err();
        assert!(error.message.contains("constant"));

        let error = execute("len(1, 2);").unwrap_err();
        assert!(error.message.contains("expected 1 arguments"));

        let error = execute("split(\"abc\", \"\");").unwrap_err();
        assert!(error.message.contains("separator cannot be empty"));

        let error = execute("sqrt(-1);").unwrap_err();
        assert!(error.message.contains("negative"));
    }
}
