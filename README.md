# mix
一个使用 Rust 实现的小型脚本语言。

这个项目主要用于学习一门编程语言内部是如何工作的，包括：

* 词法分析
* Token 生成
* 语法分析
* AST 构建
* 解释执行
* 运行时环境
* 后续的字节码和虚拟机

## 当前进度

目前已经完成：

* [x] Token 类型定义
* [x] 词法分析
* [x] Parser
* [x] AST
* [x] 表达式求值
* [x] 变量系统
* [x] 控制流
* [x] 函数
* [x] Runtime Environment
* [x] 内置函数
* [ ] REPL
* [ ] Bytecode VM

## 示例

计划支持类似下面的语法：

```text
let name = "mix";

fn add(a, b) {
    return a + b;
}

if name == "Rust" {
    print(add(10, 20));
}

for i in range(0, 10) {
    print(i);
}
```

Runtime 使用动态类型和词法作用域，支持闭包、递归函数、Python 风格真值判断、
负数索引以及字符串和数组迭代。内置函数包括 `print()`、`len()` 和 `range()`。

对象可以使用标识符或字符串作为键：

```text
let user = {name: "mix", "version": 1};
user.version += 1;
print(user.name, user["version"]);
```
