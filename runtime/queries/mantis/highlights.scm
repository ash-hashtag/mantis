; Tree-sitter highlight queries for the Mantis grammar.

(comment) @comment

(string) @string
(char) @constant.character
(number) @constant.numeric
(float) @constant.numeric.float
(boolean) @constant.builtin.boolean

[
  (fn) (let) (mut) (return)
  (if) (elif) (else)
  (loop) (break) (continue)
  (type) (struct) (enum)
  (trait) (impl)
  (extern) (import) (use)
  (async) (static) (const)
] @keyword

(function_declaration name: (identifier) @function)

(call_expression
  function: (identifier) @function.call)
(call_expression
  function: (field_expression
    field: (identifier) @function.call))

(field_expression
  object: (expression)
  field: (identifier) @variable.member)

(parameter
  (identifier) @variable.parameter)

(type_expression
  (identifier) @type)

((identifier) @type
  (#match? @type "^[A-Z]"))
