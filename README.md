# mix

`mix` 是一个使用 Rust 编写的小型动态脚本语言，用于学习编程语言从源代码到执行结果的完整过程。

项目目前包含 Lexer、Parser、AST 和树遍历解释器。Runtime 采用动态类型与词法作用域，部分行为参考 Python，同时保留 `let`、`fn`、花括号和分号等 Mix 自己的语法。


## 运行

需要安装支持 Rust 2024 Edition 的 Rust 工具链。

```shell
cargo run -- testdata/hello/hello.mix
```

运行测试和静态检查：

```shell
cargo test
cargo clippy --all-targets -- -D warnings
```

## 快速示例

```text
let name = "mix";
let numbers = [1, 2, 3, 4];

fn sum(values) {
    let total = 0;
    for value in values {
        total += value;
    }
    return total;
}

if name == "mix" {
    print(name, sum(numbers));
} else {
    print("unknown language");
}
```

输出：

```text
mix 10
```

## 语法

### 值与变量

Mix 使用动态类型。当前支持 `int`、`float`、`bool`、`string`、`null`、数组、对象和函数。

```text
let count = 10;
let ratio = 1.5;
let enabled = true;
let message = "hello\nworld";
let empty = null;

const version = 1;
```

`let` 创建可重新赋值的绑定，`const` 创建不可重新绑定的绑定。同一子作用域可以遮蔽外层名称。

### 数组

数组是可变的，并使用共享引用语义。支持负数索引，行为类似 Python list。

```text
let values = [10, 20, 30];
let alias = values;

alias[-1] = 99;
values[0] += 5;

print(values); // [15, 20, 99]
```

### 对象

对象键可以写成标识符或字符串，读取和赋值可以使用成员或下标形式。

```text
let user = {
    name: "mix",
    "version": 1,
};

user.version += 1;
user["enabled"] = true;

print(user.name, user["version"]);
```

### 运算

```text
let arithmetic = 1 + 2 * 3;
let comparison = arithmetic >= 7;
let logical = comparison && enabled;
let shifted = 1 << 4;
```

支持的运算符：

- 算术：`+ - * / %`
- 比较：`== != < <= > >=`
- 逻辑：`! && ||`
- 位运算：`& | ^ ~ << >>`
- 赋值：`= += -= *= /= %=`

`/` 总是产生浮点数。`&&` 和 `||` 会短路，并像 Python 的 `and`、`or` 一样返回其中一个操作数。字符串与数组支持 `+`，并支持使用整数进行 `*` 重复。

### 条件与循环

条件不强制使用括号。零、空字符串、空数组、空对象、`false` 和 `null` 都是假值。

```text
if score >= 90 {
    print("A");
} else if score >= 60 {
    print("B");
} else {
    print("C");
}

let count = 3;
while count > 0 {
    print(count);
    count -= 1;
}

for i in range(0, 10, 2) {
    if i == 4 {
        continue;
    }
    if i == 8 {
        break;
    }
    print(i);
}
```

数组、字符串和对象都可以用于 `for / in`。遍历对象时会得到排序后的键。

### 函数与闭包

函数使用词法作用域，可以递归，也可以捕获声明位置的变量。

```text
fn factorial(n) {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}

fn make_adder(x) {
    fn add(y) {
        return x + y;
    }
    return add;
}

let add10 = make_adder(10);
print(factorial(5)); // 120
print(add10(7));     // 17
```

没有显式 `return` 的函数返回 `null`。

### 内置函数

#### `print(...values)`

使用空格分隔并输出任意数量的值，返回 `null`。

#### `len(value)`

返回字符串的 Unicode 字符数，或者数组、对象的元素数量。

#### `range(stop)` / `range(start, stop)` / `range(start, stop, step)`

创建整数序列。参数必须是整数，`step` 不能为零。

```text
print(range(4));          // [0, 1, 2, 3]
print(range(2, 6));       // [2, 3, 4, 5]
print(range(5, 0, -2));   // [5, 3, 1]
```

### 注释

目前支持 `//` 单行注释：

```text
// 整行注释
let value = 10; // 行尾注释
```

## 错误处理

Parser 会报告错误 Token 的源码字节范围，命令行入口会将其转换成行号和列号。Runtime 会检测常见错误，包括：

- 使用未定义名称
- 给 `const` 重新赋值
- 不支持的类型运算
- 除零或模零
- 数组和字符串索引越界
- 调用非函数值或参数数量错误
- 在错误位置使用 `return`、`break` 或 `continue`

程序发生错误时会向标准错误输出信息，并以非零状态退出。

## 项目结构

```text
src/
├── lexer.rs         # Token 扫描
├── lexer/token.rs   # TokenKind、Token、Span
├── lexer/dfa.rs     # DFA 状态定义
├── parser.rs        # 递归下降 Parser
├── ast.rs           # AST 节点
├── value.rs         # 动态值、作用域和闭包环境
├── runtime.rs       # 树遍历解释器
└── main.rs          # 命令行入口
```

## 当前限制

- 只支持 `//` 单行注释。
- 标识符目前仅支持 ASCII 字母、数字和下划线。
- 函数参数暂不支持默认值、关键字参数和可变参数。
- 没有类、模块、异常处理或标准库模块。
- `range()` 当前会创建数组，而不是像 Python 一样返回惰性迭代器。
- Runtime 错误目前没有调用栈和源码行号。
