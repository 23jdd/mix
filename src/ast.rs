use crate::lexer::token::TokenKind;

/// 一个源文件解析后的根节点。
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// 代码块本质上是一组顺序执行的语句。
pub type Block = Vec<Statement>;

/// 语句节点。控制流的主体直接保存为 [`Block`]，方便后续解释器执行。
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// `let name = value;` 或 `const name = value;`。
    Variable {
        /// `let` 为 true，`const` 为 false。
        mutable: bool,
        name: String,
        value: Expression,
    },
    /// `fn name(parameters) { ... }` 函数声明。
    Function {
        name: String,
        parameters: Vec<String>,
        body: Block,
    },
    /// `if`、`else if`、`else` 条件分支。
    If {
        condition: Expression,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    /// `while condition { ... }` 循环。
    While { condition: Expression, body: Block },
    /// `for name in iterable { ... }` 遍历循环。
    For {
        name: String,
        iterable: Expression,
        body: Block,
    },
    /// `return;` 或 `return expression;`。
    Return(Option<Expression>),
    /// 提前结束当前循环。
    Break,
    /// 跳过当前循环的剩余部分，进入下一轮。
    Continue,
    /// 以分号结尾的表达式，例如函数调用或赋值。
    Expression(Expression),
    /// 独立出现的 `{ ... }` 代码块。
    Block(Block),
}

/// 表达式节点。递归子表达式使用 Box，保证枚举本身大小固定。
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// 十进制整数。词法文本在 Parser 中转换为 i64。
    Integer(i64),
    /// 十进制浮点数。词法文本在 Parser 中转换为 f64。
    Float(f64),
    /// 已处理转义字符、不包含两侧引号的字符串值。
    String(String),
    Boolean(bool),
    Null,
    /// 变量名、函数名等名称引用。
    Identifier(String),
    /// 数组字面量，例如 `[1, 2, 3]`。
    Array(Vec<Expression>),
    /// 对象字面量，例如 `{name: "mix", "version": 1}`。
    Object(Vec<(String, Expression)>),
    /// 前缀一元运算，如 `!ready`、`-number`。
    Unary {
        operator: TokenKind,
        operand: Box<Expression>,
    },
    /// 普通二元运算，运算符优先级由 Parser 处理。
    Binary {
        left: Box<Expression>,
        operator: TokenKind,
        right: Box<Expression>,
    },
    /// 赋值表达式。目标只能是标识符、索引或成员访问。
    Assignment {
        target: Box<Expression>,
        operator: TokenKind,
        value: Box<Expression>,
    },
    /// 函数调用。callee 本身也是表达式，因此支持 `factory()()`。
    Call {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    /// 下标访问，例如 `items[index]`。
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },
    /// 成员访问，例如 `user.name`。
    Member {
        object: Box<Expression>,
        property: String,
    },
}

impl Expression {
    /// 判断表达式能否出现在赋值运算符左侧。
    pub fn is_assignable(&self) -> bool {
        matches!(
            self,
            Self::Identifier(_) | Self::Index { .. } | Self::Member { .. }
        )
    }
}
