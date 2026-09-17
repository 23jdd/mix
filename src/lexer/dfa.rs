#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Start,

    // identifier
    Identifier,

    // number
    Integer,
    FloatDot, // 123.
    Float,    // 123.456

    // string
    String,
    StringEscape,

    // operators
    MaybeEqual, // + +=, * *=, / /=, % %=, ! !=
    Minus,      // - -= ->
    Equal,      // = == =>
    Less,       // < <= <<
    Greater,    // > >= >>
    AndDoubleSame, // & &&
    OrDoubleSame,  // | ||

    // ^ ~ ( ) { } [ ] , . : ; ?
    Single,

    // invalid
    Dead,
}

impl State {
    /// 返回:
    ///
    /// (next_state, reconsume)
    ///
    /// reconsume = false
    ///     当前字符 c 已经被消费。
    ///
    /// reconsume = true
    ///     当前 token 已结束，
    ///     但是当前字符 c 属于下一个 token，
    ///     lexer 必须重新把 c 交给 State::Start。
    pub fn next(self, c: char) -> (State, bool) {
        match self {
            // ============================================================
            // Start
            // ============================================================
            State::Start => match c {
                // identifier
                'a'..='z' | 'A'..='Z' | '_' => {
                    (State::Identifier, false)
                }

                // number
                '0'..='9' => {
                    (State::Integer, false)
                }

                // string
                '"' => {
                    (State::String, false)
                }

                // + +=
                // * *=
                // / /=
                // % %=
                // ! !=
                '+' | '*' | '/' | '%' | '!' => {
                    (State::MaybeEqual, false)
                }

                // - -= ->
                '-' => {
                    (State::Minus, false)
                }

                // = == =>
                '=' => {
                    (State::Equal, false)
                }

                // < <= <<
                '<' => {
                    (State::Less, false)
                }

                // > >= >>
                '>' => {
                    (State::Greater, false)
                }

                // & &&
                '&' => {
                    (State::AndDoubleSame, false)
                }

                // | ||
                '|' => {
                    (State::OrDoubleSame, false)
                }

                // always single char
                '^'
                | '~'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']'
                | ','
                | '.'
                | ':'
                | ';'
                | '?' => {
                    (State::Single, false)
                }

                // whitespace
                ' ' | '\t' | '\n' | '\r' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Dead, false)
                }
            },

            // ============================================================
            // Identifier
            //
            // abc
            // hello123
            // _name
            // foo_bar_123
            // ============================================================
            State::Identifier => match c {
                'a'..='z'
                | 'A'..='Z'
                | '0'..='9'
                | '_' => {
                    (State::Identifier, false)
                }

                _ if is_delimiter(c) => {
                    (State::Start, true)
                }

                _ => {
                    (State::Dead, false)
                }
            },

            // ============================================================
            // Integer
            //
            // 1
            // 123
            // ============================================================
            State::Integer => match c {
                '0'..='9' => {
                    (State::Integer, false)
                }

                '.' => {
                    (State::FloatDot, false)
                }

                // 禁止：
                //
                // 123abc
                // 123_name
                //
                // 不应该解析成:
                // Integer(123) + Identifier(abc)
                'a'..='z' | 'A'..='Z' | '_' => {
                    (State::Dead, false)
                }

                _ if is_delimiter(c) => {
                    (State::Start, true)
                }

                _ => {
                    (State::Dead, false)
                }
            },

            // ============================================================
            // FloatDot
            //
            // 已经读取:
            //
            // 123.
            //
            // 必须至少再读取一个 digit。
            // ============================================================
            State::FloatDot => match c {
                '0'..='9' => {
                    (State::Float, false)
                }

                _ => {
                    // 123. 不允许
                    (State::Dead, false)
                }
            },

            // ============================================================
            // Float
            //
            // 1.0
            // 123.456
            // ============================================================
            State::Float => match c {
                '0'..='9' => {
                    (State::Float, false)
                }

                // 禁止:
                //
                // 1.2abc
                // 1.2_foo
                // 1.2.3
                'a'..='z' | 'A'..='Z' | '_' | '.' => {
                    (State::Dead, false)
                }

                _ if is_delimiter(c) => {
                    (State::Start, true)
                }

                _ => {
                    (State::Dead, false)
                }
            },

            // ============================================================
            // String
            //
            // "hello"
            // "hello world"
            // ============================================================
            State::String => match c {
                // string end
                '"' => {
                    (State::Start, false)
                }

                // escape
                '\\' => {
                    (State::StringEscape, false)
                }

                // 普通字符串禁止真正换行
                '\n' | '\r' => {
                    (State::Dead, false)
                }

                _ => {
                    (State::String, false)
                }
            },

            // ============================================================
            // StringEscape
            //
            // \"
            // \\
            // \n
            // \r
            // \t
            // \0
            // ============================================================
            State::StringEscape => match c {
                '"'
                | '\\'
                | 'n'
                | 'r'
                | 't'
                | '0' => {
                    (State::String, false)
                }

                _ => {
                    (State::Dead, false)
                }
            },

            // ============================================================
            // MaybeEqual
            //
            // +
            // +=
            //
            // *
            // *=
            //
            // /
            // /=
            //
            // %
            // %=
            //
            // !
            // !=
            // ============================================================
            State::MaybeEqual => match c {
                '=' => {
                    (State::Start, false)
                }

                _ => {
                    // 前面的 operator 自己就是完整 token
                    //
                    // 当前 c 属于下一个 token
                    (State::Start, true)
                }
            },

            // ============================================================
            // Minus
            //
            // -
            // -=
            // ->
            // ============================================================
            State::Minus => match c {
                '=' | '>' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Start, true)
                }
            },

            // ============================================================
            // Equal
            //
            // =
            // ==
            // =>
            // ============================================================
            State::Equal => match c {
                '=' | '>' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Start, true)
                }
            },

            // ============================================================
            // Less
            //
            // <
            // <=
            // <<
            // ============================================================
            State::Less => match c {
                '=' | '<' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Start, true)
                }
            },

            // ============================================================
            // Greater
            //
            // >
            // >=
            // >>
            // ============================================================
            State::Greater => match c {
                '=' | '>' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Start, true)
                }
            },

            // ============================================================
            // &
            // &&
            // ============================================================
            State::AndDoubleSame => match c {
                '&' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Start, true)
                }
            },

            // ============================================================
            // |
            // ||
            // ============================================================
            State::OrDoubleSame => match c {
                '|' => {
                    (State::Start, false)
                }

                _ => {
                    (State::Start, true)
                }
            },

            // ============================================================
            // Single
            //
            // ^
            // ~
            // (
            // )
            // {
            // }
            // [
            // ]
            // ,
            // .
            // :
            // ;
            // ?
            // ============================================================
            State::Single => {
                // 当前字符不是 Single 的一部分，
                // Single token 已经在上一次 transition 消费完成。
                (State::Start, true)
            }

            // ============================================================
            // Dead
            // ============================================================
            State::Dead => {
                (State::Dead, false)
            }
        }
    }

    /// EOF 时检查当前 State 是否是合法结束状态。
    ///
    /// 例如：
    ///
    /// abc EOF      -> OK
    /// 123 EOF      -> OK
    /// 1.23 EOF     -> OK
    /// + EOF        -> OK
    ///
    /// 123. EOF     -> Error
    /// "hello EOF   -> Error
    /// "abc\ EOF    -> Error
    pub fn can_finish(self) -> bool {
        matches!(
            self,
            State::Start
                | State::Identifier
                | State::Integer
                | State::Float
                | State::MaybeEqual
                | State::Minus
                | State::Equal
                | State::Less
                | State::Greater
                | State::AndDoubleSame
                | State::OrDoubleSame
                | State::Single
        )
    }

    /// 当前 state 是否表示 lexer 处在错误状态。
    pub fn is_dead(self) -> bool {
        matches!(self, State::Dead)
    }
}

/// identifier 中合法字符
pub fn is_identifier_continue(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_')
}

/// token 之间合法的分隔字符。
///
/// 注意：
///
/// identifier 后面遇到 operator / punctuation / whitespace
/// 意味着 identifier 已完成。
fn is_delimiter(c: char) -> bool {
    matches!(
        c,
        // operators
        '+'
            | '-'
            | '*'
            | '/'
            | '%'
            | '!'
            | '='
            | '<'
            | '>'
            | '&'
            | '|'
            | '^'
            | '~'

            // punctuation
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | ','
            | '.'
            | ':'
            | ';'
            | '?'

            // string start
            | '"'

            // whitespace
            | ' '
            | '\t'
            | '\n'
            | '\r'
    )
}