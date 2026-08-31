use super::{AnalysisStatus, Issue, Span, MAX_DEPTH, MAX_NODES};

#[derive(Clone, Debug)]
pub(super) struct Word {
    pub value: String,
    pub dynamic: bool,
    pub executes: bool,
    pub quoted: bool,
    pub assignment: bool,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub(super) struct Redirect {
    pub op: String,
    pub fd: Option<String>,
    pub word: Word,
    pub body: Option<Word>,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub(super) enum Token {
    Word(Word),
    Redirect(Redirect),
    Op(String, Span),
}
impl Token {
    pub(super) fn span(&self) -> Span {
        match self {
            Self::Word(w) => w.span,
            Self::Redirect(r) => r.span,
            Self::Op(_, s) => *s,
        }
    }
}

pub(super) fn lex(source: &str) -> Result<Vec<Token>, Issue> {
    Lexer {
        source,
        pos: 0,
        tokens: Vec::new(),
        pending: Vec::new(),
    }
    .run()
}
struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    pending: Vec<usize>,
}
impl Lexer<'_> {
    fn issue(&self, reason: &'static str) -> Issue {
        Issue {
            status: AnalysisStatus::Invalid,
            reason,
            span: Span {
                start: self.pos,
                end: self.pos,
            },
        }
    }
    fn rest(&self) -> &str {
        &self.source[self.pos..]
    }
    fn ch(&self) -> Option<char> {
        self.rest().chars().next()
    }
    fn bump(&mut self) -> char {
        let c = self.ch().unwrap();
        self.pos += c.len_utf8();
        c
    }
    fn run(mut self) -> Result<Vec<Token>, Issue> {
        while self.pos < self.source.len() {
            if self.tokens.len() >= MAX_NODES {
                return Err(Issue {
                    status: AnalysisStatus::ResourceLimit,
                    reason: "shell token limit",
                    span: Span {
                        start: self.pos,
                        end: self.pos,
                    },
                });
            }
            match self.ch().unwrap() {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '\n' => {
                    let start = self.pos;
                    self.bump();
                    self.tokens.push(Token::Op(
                        "\n".into(),
                        Span {
                            start,
                            end: self.pos,
                        },
                    ));
                    self.heredocs()?;
                }
                '#' => {
                    while self.ch().is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                }
                '\\' if self.rest().starts_with("\\\n") => {
                    self.pos += 2;
                }
                _ => {
                    let start = self.pos;
                    // An IO number is recognized only when immediately followed by a redirect.
                    let digits = self.rest().bytes().take_while(u8::is_ascii_digit).count();
                    let at = self.pos + digits;
                    let tail = &self.source[at..];
                    let redir = [
                        "&>>", "<<<", "<<-", ">>", ">|", "<>", ">&", "<&", "<<", "&>", ">", "<",
                    ]
                    .into_iter()
                    .find(|op| tail.starts_with(op));
                    if let Some(op) =
                        redir.filter(|_| !tail.starts_with("<(") && !tail.starts_with(">("))
                    {
                        let fd = (digits > 0).then(|| self.source[self.pos..at].to_owned());
                        self.pos = at + op.len();
                        // Bash removes escaped newlines before recognizing the operator.
                        // Keep the original byte coordinates while joining the FD operator.
                        let mut operator = op.to_owned();
                        while self.rest().starts_with("\\\n") {
                            self.pos += 2;
                        }
                        if matches!(op, ">" | "<") && self.ch() == Some('&') {
                            self.bump();
                            operator.push('&');
                        }
                        while matches!(self.ch(), Some(' ' | '\t')) {
                            self.bump();
                        }
                        let word = self.word(operator == "<<" || operator == "<<-")?;
                        let span = Span {
                            start,
                            end: self.pos,
                        };
                        let pending = operator == "<<" || operator == "<<-";
                        if pending {
                            self.pending.push(self.tokens.len());
                        }
                        self.tokens.push(Token::Redirect(Redirect {
                            op: operator,
                            fd,
                            word,
                            body: None,
                            span,
                        }));
                    } else if let Some(op) =
                        ["&&", "||", "|&", ";;&", ";;", ";&", ";", "|", "&", "(", ")"]
                            .into_iter()
                            .find(|op| self.rest().starts_with(op))
                    {
                        self.pos += op.len();
                        self.tokens.push(Token::Op(
                            op.into(),
                            Span {
                                start,
                                end: self.pos,
                            },
                        ));
                    } else {
                        let word = self.word(false)?;
                        self.tokens.push(Token::Word(word));
                    }
                }
            }
        }
        if !self.pending.is_empty() {
            return Err(self.issue("unterminated here-doc"));
        }
        Ok(self.tokens)
    }
    fn word(&mut self, delimiter: bool) -> Result<Word, Issue> {
        let start = self.pos;
        let mut value = String::new();
        let mut quoted = false;
        let mut dynamic = false;
        let mut executes = false;
        let mut quote = None;
        loop {
            let Some(c) = self.ch() else {
                break;
            };
            if quote == Some('\'') {
                self.bump();
                if c == '\'' {
                    quote = None;
                } else {
                    value.push(c);
                }
                continue;
            }
            if c == '\\' {
                quoted = true;
                self.bump();
                let Some(next) = self.ch() else {
                    return Err(self.issue("trailing escape"));
                };
                if next == '\n' {
                    self.bump();
                    continue;
                }
                if quote == Some('"') && !matches!(next, '$' | '`' | '"' | '\\') {
                    value.push('\\');
                } else {
                    value.push(self.bump());
                    continue;
                }
            } else if c == '\'' && quote.is_none() {
                quoted = true;
                quote = Some('\'');
                self.bump();
                continue;
            } else if c == '"' {
                quoted = true;
                quote = if quote.is_none() { Some('"') } else { None };
                self.bump();
                continue;
            }
            if !delimiter
                && (self.rest().starts_with("$(")
                    || (quote.is_none()
                        && (self.rest().starts_with("<(") || self.rest().starts_with(">("))))
            {
                dynamic = true;
                executes = true;
                self.pos += 2;
                self.skip_substitution(1)?;
                continue;
            }
            if !delimiter && c == '`' {
                dynamic = true;
                executes = true;
                self.bump();
                loop {
                    match self.ch() {
                        None => return Err(self.issue("unterminated command substitution")),
                        Some('`') => {
                            self.bump();
                            break;
                        }
                        Some('\\') => {
                            self.bump();
                            if self.ch().is_some() {
                                self.bump();
                            }
                        }
                        _ => {
                            self.bump();
                        }
                    }
                }
                continue;
            }
            if quote.is_none() && (c.is_whitespace() || "<>|&;()".contains(c)) {
                break;
            }
            if !delimiter
                && (c == '$'
                    || (quote.is_none()
                        && ("*?[{}".contains(c) || (c == '~' && self.pos == start))))
            {
                dynamic = true;
            }
            // Braces/control syntax and unsupported quote forms never decode as literal targets.
            value.push(self.bump());
        }
        if quote.is_some() {
            return Err(self.issue("unterminated quote"));
        }
        if self.pos == start {
            return Err(self.issue("missing redirection operand or word"));
        }
        let raw = &self.source[start..self.pos];
        let assignment = raw.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty()
                && name.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
        });
        Ok(Word {
            value,
            dynamic,
            executes,
            quoted,
            assignment,
            span: Span {
                start,
                end: self.pos,
            },
        })
    }
    fn skip_substitution(&mut self, mut depth: usize) -> Result<(), Issue> {
        let mut quote = None;
        while let Some(c) = self.ch() {
            self.bump();
            if c == '\\' {
                if self.ch().is_some() {
                    self.bump();
                }
                continue;
            }
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
                continue;
            }
            if matches!(c, '\'' | '"' | '`') {
                quote = Some(c);
                continue;
            }
            if c == '(' {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(Issue {
                        status: AnalysisStatus::ResourceLimit,
                        reason: "shell nesting limit",
                        span: Span {
                            start: self.pos,
                            end: self.pos,
                        },
                    });
                }
            }
            if c == ')' {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
        }
        Err(self.issue("unterminated substitution"))
    }
    fn heredocs(&mut self) -> Result<(), Issue> {
        for index in std::mem::take(&mut self.pending) {
            let Token::Redirect(r) = &self.tokens[index] else {
                unreachable!()
            };
            let delimiter = r.word.value.clone();
            let strip = r.op == "<<-";
            let quoted = r.word.quoted;
            let start = self.pos;
            let mut body = String::new();
            let mut found = false;
            while self.pos < self.source.len() {
                let line_start = self.pos;
                let line_end = self
                    .rest()
                    .find('\n')
                    .map(|n| self.pos + n)
                    .unwrap_or(self.source.len());
                let line = &self.source[line_start..line_end];
                let line = if strip {
                    line.trim_start_matches('\t')
                } else {
                    line
                };
                self.pos = if line_end < self.source.len() {
                    line_end + 1
                } else {
                    line_end
                };
                if line == delimiter {
                    found = true;
                    break;
                }
                body.push_str(line);
                if line_end < self.source.len() {
                    body.push('\n');
                }
            }
            if !found {
                return Err(self.issue("unterminated here-doc"));
            }
            let expansion = !quoted && has_unescaped_expansion(&body);
            let executes = !quoted && has_executable_expansion(&body);
            let Token::Redirect(r) = &mut self.tokens[index] else {
                unreachable!()
            };
            r.body = Some(Word {
                value: body,
                dynamic: expansion,
                executes,
                quoted,
                assignment: false,
                span: Span {
                    start,
                    end: self.pos,
                },
            });
        }
        Ok(())
    }
}
fn has_unescaped_expansion(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == '$' || c == '`' {
            return true;
        }
    }
    false
}
fn has_executable_expansion(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c == '`' || (c == '$' && chars.peek() == Some(&'(')) {
            return true;
        }
    }
    false
}
