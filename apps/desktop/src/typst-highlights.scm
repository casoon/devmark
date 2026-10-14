; Typst-Highlighting für die Grammatik uben0/tree-sitter-typst, abgebildet auf die
; Highlight-Namen des gpui-kit-Themes. Frühere Muster haben Vorrang.

(comment) @comment

(call item: (ident) @function)
(call item: (field field: (ident) @function))
(tagged field: (ident) @property)
(field field: (ident) @property)

(let "let" @keyword)
(branch ["if" "else"] @keyword)
(while "while" @keyword)
(for ["for" "in"] @keyword)
(import "import" @keyword)
(as "as" @keyword)
(include "include" @keyword)
(show "show" @keyword)
(set "set" @keyword)
(return "return" @keyword)
(flow ["break" "continue"] @keyword)
(context "context" @keyword)
(in ["in" "not"] @keyword)
(and "and" @keyword)
(or "or" @keyword)
(not "not" @keyword)

(sign ["+" "-"] @operator)
(add "+" @operator)
(sub "-" @operator)
(mul "*" @operator)
(div "/" @operator)
(cmp ["==" "<=" ">=" "!=" "<" ">"] @operator)
(fraction "/" @operator)
(fac "!" @operator)
(attach ["^" "_"] @operator)
(wildcard) @operator

(raw_blck) @text.literal
(raw_span) @text.code.span
(label) @label
(ref) @label
(number) @number
(string) @string
(bool) @boolean
(none) @constant
(auto) @constant

(item "-" @punctuation.list_marker)
(term ["/" ":"] @punctuation.list_marker)
(heading) @title
(url) @link_uri
(emph) @emphasis
(strong) @emphasis.strong
(quote) @string.special
(shorthand) @constant
(linebreak) @constant
(escape) @string.escape
(math) @string.special.symbol

"#" @punctuation.special
["(" ")" "{" "}"] @punctuation.bracket
(content ["[" "]"] @punctuation.bracket)
["," ";" ".." ":"] @punctuation.delimiter

(ident) @variable
