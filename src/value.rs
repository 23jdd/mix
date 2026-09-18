use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::Block;

/// 一个词法作用域。clone 只复制 Rc，因此闭包和调用者看到的是同一份环境。
#[derive(Clone)]
pub struct Environment(Rc<RefCell<Scope>>);

struct Scope {
    values: HashMap<String, Binding>,
    parent: Option<Environment>,
}

#[derive(Clone)]
struct Binding {
    mutable: bool,
    value: Value,
}

impl Environment {
    pub fn global() -> Self {
        Self(Rc::new(RefCell::new(Scope {
            values: HashMap::new(),
            parent: None,
        })))
    }

    pub fn child(&self) -> Self {
        Self(Rc::new(RefCell::new(Scope {
            values: HashMap::new(),
            parent: Some(self.clone()),
        })))
    }

    /// 只检查当前作用域，允许在子作用域遮蔽外层同名变量。
    pub fn define(&self, name: String, value: Value, mutable: bool) -> Result<(), String> {
        let mut scope = self.0.borrow_mut();
        if scope.values.contains_key(&name) {
            return Err(format!("name `{name}` is already defined in this scope"));
        }
        scope.values.insert(name, Binding { mutable, value });
        Ok(())
    }

    /// 从当前作用域开始沿 parent 链查找变量。
    pub fn get(&self, name: &str) -> Option<Value> {
        let parent = {
            let scope = self.0.borrow();
            if let Some(binding) = scope.values.get(name) {
                return Some(binding.value.clone());
            }
            scope.parent.clone()
        };
        parent.and_then(|parent| parent.get(name))
    }

    /// 找到变量真正所属的作用域，供赋值操作使用。
    pub fn resolve(&self, name: &str) -> Option<Self> {
        let parent = {
            let scope = self.0.borrow();
            if scope.values.contains_key(name) {
                return Some(self.clone());
            }
            scope.parent.clone()
        };
        parent.and_then(|parent| parent.resolve(name))
    }

    pub fn get_local(&self, name: &str) -> Option<Value> {
        self.0
            .borrow()
            .values
            .get(name)
            .map(|binding| binding.value.clone())
    }

    pub fn assign_local(&self, name: &str, value: Value) -> Result<(), String> {
        let mut scope = self.0.borrow_mut();
        let binding = scope
            .values
            .get_mut(name)
            .ok_or_else(|| format!("undefined name `{name}`"))?;
        if !binding.mutable {
            return Err(format!("cannot assign to constant `{name}`"));
        }
        binding.value = value;
        Ok(())
    }
}

/// 用户函数保存声明时的环境，从而实现词法闭包。
#[derive(Clone)]
pub struct FunctionValue {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Block,
    pub closure: Environment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Print,
    Len,
    Range,
    ReadFile,
    WriteFile,
    AppendFile,
    FileExists,
    ListDir,
}

/// Runtime 中流动的动态值。
///
/// Array/Object 使用共享可变容器，因此把数组传给函数后，索引赋值会修改同一对象，
/// 与 Python 的可变 list/dict 引用语义一致。
#[derive(Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Rc<RefCell<Vec<Value>>>),
    Object(Rc<RefCell<HashMap<String, Value>>>),
    Function(Rc<FunctionValue>),
    Builtin(Builtin),
}

impl Value {
    /// Python 风格真值判断：零、空容器、空字符串和 null 为假。
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(value) => *value,
            Self::Int(value) => *value != 0,
            Self::Float(value) => *value != 0.0,
            Self::String(value) => !value.is_empty(),
            Self::Array(value) => !value.borrow().is_empty(),
            Self::Object(value) => !value.borrow().is_empty(),
            Self::Function(_) | Self::Builtin(_) => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
            Self::Function(_) | Self::Builtin(_) => "function",
        }
    }

    /// print 顶层字符串不加引号，容器内的字符串使用 repr 风格。
    pub fn display(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            _ => self.repr(),
        }
    }

    pub fn repr(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(true) => "true".into(),
            Self::Bool(false) => "false".into(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => {
                let mut text = value.to_string();
                if !text.contains('.') && !text.contains('e') {
                    text.push_str(".0");
                }
                text
            }
            Self::String(value) => format!("{value:?}"),
            Self::Array(values) => {
                let elements = values
                    .borrow()
                    .iter()
                    .map(Self::repr)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{elements}]")
            }
            Self::Object(values) => {
                let mut entries = values
                    .borrow()
                    .iter()
                    .map(|(key, value)| format!("{key:?}: {}", value.repr()))
                    .collect::<Vec<_>>();
                entries.sort();
                format!("{{{}}}", entries.join(", "))
            }
            Self::Function(function) => format!("<fn {}>", function.name),
            Self::Builtin(builtin) => format!("<builtin {}>", builtin.name()),
        }
    }
}

impl Builtin {
    pub fn name(self) -> &'static str {
        match self {
            Self::Print => "print",
            Self::Len => "len",
            Self::Range => "range",
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::AppendFile => "append_file",
            Self::FileExists => "file_exists",
            Self::ListDir => "list_dir",
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.repr())
    }
}
